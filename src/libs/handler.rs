use eyre::*;
use futures::future::BoxFuture;
use futures::FutureExt;
use serde_json::Value;
use std::result::Result::Ok;

use super::deserializer_wrapper;
use super::error_code::ErrorCode;
use super::toolbox::{ArcToolbox, RequestContext};
use super::ws::{request_error_to_resp, WsRequest};

#[allow(type_alias_bounds)]
pub type FutureResponse<T: WsRequest> = BoxFuture<'static, Result<T::Response>>;

pub trait RequestHandler: Send + Sync {
    type Request: WsRequest + 'static;
    fn handle(
        &self,
        toolbox: &ArcToolbox,
        ctx: RequestContext,
        req: Self::Request,
    ) -> FutureResponse<Self::Request>;
}

pub trait RequestHandlerErased: Send + Sync {
    fn handle(
        &self,
        toolbox: &ArcToolbox,
        ctx: RequestContext,
        req: Value,
    ) -> BoxFuture<'static, ()>;
}

impl<T: RequestHandler> RequestHandlerErased for T {
    fn handle(
        &self,
        toolbox: &ArcToolbox,
        ctx: RequestContext,
        req: Value,
    ) -> BoxFuture<'static, ()> {
        // TODO: find a better way to avoid double parsing or serialization

        // TODO: test this approach
        let des = &mut deserializer_wrapper::Deserializer::from_value(&req);
        let data: Result<T::Request, _> = serde_path_to_error::deserialize(des);
        let data: T::Request = match data {
            Ok(data) => data,
            Err(err) => {
                let path = err.path().to_string();
                toolbox.send(
                    ctx.connection_id,
                    request_error_to_resp(
                        &ctx,
                        ErrorCode::new(100400), // Bad Request
                        format!("{}: {}", path, err),
                    ),
                );
                return async { () }.boxed();
            }
        };

        // TODO: find a better way to avoid double parsing or serialization

        // let buf = serde_json::to_string(&req).unwrap();
        // let data: T::Request = match serde_json::from_value(req) {
        //     Ok(data) => data,
        //     Err(err) => {
        //         let jd = &mut serde_json::Deserializer::from_str(&buf);
        //         let data: Result<T::Request, _> = serde_path_to_error::deserialize(jd);
        //         let path = data.err().map(|err| err.path().to_string());
        //         toolbox.send(
        //             ctx.connection_id,
        //             request_error_to_resp(
        //                 &ctx,
        //                 ErrorCode::new(100400), // Bad Request
        //                 if let Some(path) = path {
        //                     format!("{}: {}", path, err)
        //                 } else {
        //                     format!("{}", err)
        //                 },
        //             ),
        //         );
        //         return async { () }.boxed();
        //     }
        // };

        let fut = RequestHandler::handle(self, toolbox, ctx, data);
        let toolbox = toolbox.clone();
        async move {
            let resp = fut.await;
            if let Some(resp) = Toolbox::encode_ws_response(ctx, resp) {
                toolbox.send(ctx.connection_id, resp);
            }
        }
        .boxed()
    }
}
