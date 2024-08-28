use crate::handler::*;
use crate::listener::{ConnectionListener, TcpListener, TlsListener};
use crate::toolbox::{ArcToolbox, RequestContext, Toolbox};
use crate::utils::{get_conn_id, get_log_id};
use crate::ws::WsEndpoint;
use crate::ws::WsResponseValue;
use crate::ws::WsServerConfig;
use crate::ws::{check_handler, WsConnection};
use endpoint_gen::model::EndpointSchema;
use eyre::*;
use hyper::body::HttpBody;
use hyper::server::accept::Accept;
use hyper::service::service_fn;
use hyper::{Body, Request, Response, StatusCode};
use serde_json::Value;
use std::collections::HashMap;
use std::convert::Infallible;
use std::future::poll_fn;
use std::net::SocketAddr;
use std::pin::Pin;
use std::result::Result::Ok;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::task::Poll;
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::*;

pub struct HttpServer {
    pub handlers: HashMap<String, WsEndpoint>,
    pub toolbox: ArcToolbox,
    pub config: WsServerConfig,
}

impl HttpServer {
    pub fn new(config: WsServerConfig) -> Self {
        Self {
            handlers: Default::default(),
            toolbox: Toolbox::new(),
            config,
        }
    }

    pub fn add_handler<T: RequestHandler + 'static>(&mut self, schema: EndpointSchema, handler: T) {
        check_handler::<T>(&schema).expect("Invalid handler");
        self.add_handler_erased(schema, Arc::new(handler))
    }
    pub fn add_handler_erased(
        &mut self,
        schema: EndpointSchema,
        handler: Arc<dyn RequestHandlerErased>,
    ) {
        let old = self
            .handlers
            .insert(schema.name.clone(), WsEndpoint { schema, handler });
        if let Some(old) = old {
            panic!(
                "Overwriting handler for endpoint {} {}",
                old.schema.code, old.schema.name
            );
        }
    }
    async fn handle_connection<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
        self: Arc<Self>,
        addr: SocketAddr,
        stream: S,
    ) {
        let conn = Arc::new(WsConnection {
            connection_id: get_conn_id(),
            user_id: Default::default(),
            role: AtomicU32::new(0),
            address: addr,
            log_id: get_log_id(),
        });
        let mut seq = 0;
        let handler = move |req| {
            let this = Arc::clone(&self);
            seq += 1;
            let conn = Arc::clone(&conn);
            let log_id = conn.log_id;
            async move {
                match this.handle_request(conn, req, seq).await {
                    Ok(ok) => Ok::<_, Infallible>(ok),
                    Err(err) => {
                        error!("Error handling request: {:?} log_id={}", err, log_id);
                        Ok(Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(format!("Internal Server Error: log_id={}", log_id).into())
                            .unwrap())
                    }
                }
            }
        };
        let service = hyper::service::make_service_fn(move |_| {
            let handler = handler.clone();
            futures::future::ready(Ok::<_, Infallible>(service_fn(handler)))
        });

        let s = hyper::server::Server::builder(ImmediateAcceptor {
            listener: Some(stream),
        })
        .serve(service);

        if let Err(e) = s.await {
            warn!("Error serving connection: {:?}", e);
        }
    }

    pub async fn handle_request(
        self: Arc<Self>,
        conn: Arc<WsConnection>,
        request: Request<Body>,
        seq: u32,
    ) -> Result<Response<String>> {
        let url = request.uri().path().trim_start_matches("/");

        let endpoint = match self.handlers.get(url) {
            Some(endpoint) => endpoint,
            None => {
                return Ok(Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(format!("Endpoint {} not found", url))?);
            }
        };
        let context = RequestContext {
            connection_id: conn.connection_id,
            user_id: conn.get_user_id(),
            seq,
            method: endpoint.schema.code,
            log_id: conn.log_id,
            role: conn.role.load(Ordering::Relaxed),
            ip_addr: conn.address.ip(),
        };
        let mut body = vec![];
        let mut b = request.into_body();
        while let Some(chunk) = poll_fn(|cx| Pin::new(&mut b).poll_data(cx)).await {
            let chunk = chunk?;
            body.extend_from_slice(chunk.as_ref());
        }

        let req: Value = match serde_json::from_slice(&body) {
            Ok(req) => req,
            Err(err) => {
                return Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(err.to_string())?);
            }
        };
        let (tx, rx) = kanal::unbounded_async();
        let toolbox = self.toolbox.clone();
        *toolbox.send_msg.write().unwrap() = Arc::new(move |_conn, resp| {
            futures::executor::block_on(tx.send(resp)).unwrap();
            true
        });
        endpoint.handler.handle(&toolbox, context, req).await;
        let resp = rx.recv().await?;
        info!("Response: {:?}", resp);
        match resp {
            WsResponseValue::Immediate(x) => Ok(Response::builder()
                .status(StatusCode::OK)
                .body(serde_json::to_string(&x.params)?)?),
            WsResponseValue::Error(err) => Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(serde_json::to_string(&err)?)?),

            _ => {
                todo!()
            }
        }
    }

    pub async fn listen(self) -> Result<()> {
        let addr = tokio::net::lookup_host(self.config.address.as_str())
            .await
            .context("Failed to resolve address")?
            .next()
            .context("No address found")?;
        if self.config.pub_certs.is_none() && self.config.priv_key.is_none() {
            let listener = TcpListener::bind(addr).await?;
            self.listen_impl(Arc::new(listener), addr).await
        } else if self.config.pub_certs.is_some() && self.config.priv_key.is_none() {
            let listener = TcpListener::bind(addr).await?;

            let listener = TlsListener::bind(
                listener,
                self.config.pub_certs.clone().unwrap(),
                self.config.priv_key.clone().unwrap(),
            )
            .await?;
            self.listen_impl(Arc::new(listener), addr).await
        } else {
            bail!("pub_certs and priv_cert should be both set or unset")
        }
    }

    async fn listen_impl<T: ConnectionListener + 'static>(
        self,
        listener: Arc<T>,
        listen_addr: SocketAddr,
    ) -> Result<()> {
        info!("{} listening on {}", self.config.name, listen_addr);

        let this = Arc::new(self);
        loop {
            let ret = async {
                let (stream, addr) = listener.accept().await?;
                let listener2 = Arc::clone(&listener);
                let this = Arc::clone(&this);
                tokio::spawn(async move {
                    let ret: Result<()> = async {
                        let stream = listener2.handshake(stream).await?;
                        info!("Accepted stream from {}", addr);

                        this.handle_connection(addr, stream).await;
                        Ok(())
                    }
                    .await;
                    if let Err(err) = ret {
                        error!("Error while handshaking stream: {:?}", err);
                    }
                });
                Ok::<_, Error>(())
            }
            .await;
            if let Err(err) = ret {
                error!("Error while accepting stream: {:?}", err);
            }
        }
    }
}

struct ImmediateAcceptor<T> {
    listener: Option<T>,
}

impl<T: Unpin> Accept for ImmediateAcceptor<T> {
    type Conn = T;
    type Error = Infallible;

    fn poll_accept(
        mut self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<std::result::Result<Self::Conn, Self::Error>>> {
        let this = &mut *self;
        match this.listener.take() {
            Some(x) => Poll::Ready(Some(Ok(x))),
            None => Poll::Ready(None),
        }
    }
}
