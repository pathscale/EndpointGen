use dashmap::DashMap;
use std::sync::Arc;

use crate::ws::basics::WsConnection;
use crate::ws::ConnectionId;
use tokio_tungstenite::tungstenite::Message;

pub struct WebsocketStates {
    states: Arc<DashMap<ConnectionId, Arc<WsStreamState>>>,
}
impl WebsocketStates {
    pub fn new() -> Self {
        Self {
            states: Default::default(),
        }
    }
    pub fn remove(&self, connection_id: u32) {
        self.states.remove(&connection_id);
    }

    pub fn get_state(&self, connection_id: u32) -> Option<Arc<WsStreamState>> {
        self.states.get(&connection_id).map(|x| x.value().clone())
    }
    pub fn clone_states(&self) -> Arc<DashMap<u32, Arc<WsStreamState>>> {
        Arc::clone(&self.states)
    }
    pub fn insert(
        &self,
        connection_id: u32,
        message_queue: tokio::sync::mpsc::Sender<Message>,
        conn: Arc<WsConnection>,
    ) {
        self.states.insert(
            connection_id,
            Arc::new(WsStreamState {
                conn,
                message_queue,
            }),
        );
    }
}

pub struct WsStreamState {
    pub conn: Arc<WsConnection>,
    pub message_queue: tokio::sync::mpsc::Sender<Message>,
}
