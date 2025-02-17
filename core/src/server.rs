use crate::configuration::Handler;
use crate::handlers::TransportHandler;
use crate::Configuration;
use gveditor_core_api::filesystems::{
    DirItemInfo,
    FileInfo,
    FilesystemErrors,
};
use gveditor_core_api::messaging::{
    ExtensionMessages,
    Messages,
};
use gveditor_core_api::state::{
    StateData,
    StatesList,
};
use gveditor_core_api::{
    Errors,
    ManifestInfo,
    Mutex,
};
use jsonrpc_core::BoxFuture;
use jsonrpc_derive::rpc;

use std::sync::Arc;
use tokio::sync::mpsc::Receiver;

pub struct Server {
    states: Arc<Mutex<StatesList>>,
    config: Configuration,
}

impl Server {
    /// Create a new Server
    pub fn new(config: Configuration, states: Arc<Mutex<StatesList>>) -> Self {
        Self::create_receiver(
            states.clone(),
            config.receiver.clone(),
            config.handler.clone(),
        );

        Self { config, states }
    }

    /// Receive all incoming messages
    pub fn create_receiver(
        states: Arc<Mutex<StatesList>>,
        receiver: Arc<Mutex<Receiver<Messages>>>,
        handler: Handler,
    ) {
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let mut receiver = receiver.lock().await;
                loop {
                    if let Some(message) = receiver.recv().await {
                        Self::process_message(states.clone(), message, handler.clone()).await;
                    }
                }
            });
        });
    }

    /// Run the configured handler
    pub async fn run(&self) {
        let states = self.states.clone();
        let mut handler = self.config.handler.lock().await;

        handler
            .run(states.clone(), self.config.sender.clone())
            .await;
    }

    /// Process every message
    pub async fn process_message(
        states: Arc<Mutex<StatesList>>,
        msg: Messages,
        handler: Arc<Mutex<Box<dyn TransportHandler + Send + Sync>>>,
    ) {
        match msg {
            Messages::ListenToState {
                state_id,
                trigger: _,
            } => {
                // Make sure if there is already an existing state
                let state = {
                    let states = states.lock().await;
                    states.get_state_by_id(state_id)
                };

                if let Some(state) = state {
                    let handler = handler.lock().await;
                    // Send the loaded state to the handler
                    let message = Messages::StateUpdated {
                        state_data: state.lock().await.data.clone(),
                    };
                    handler.send(message).await;

                    state.lock().await.run_extensions().await;
                }
            }
            Messages::StateUpdated { .. } => {
                let states = states.lock().await;
                states
                    .notify_extensions(ExtensionMessages::CoreMessage(msg))
                    .await;
            }
            _ => {
                // Forward to the handler messages not handled here
                let handler = handler.lock().await;
                handler.send(msg).await;
            }
        }
    }
}

pub type RPCResult<T> = jsonrpc_core::Result<T>;

/// Definition of all JSON RPC Methods
#[rpc]
pub trait RpcMethods {
    #[rpc(name = "get_state_data_by_id")]
    fn get_state_by_id(
        &self,
        state_id: u8,
        token: String,
    ) -> BoxFuture<RPCResult<Option<StateData>>>;

    #[rpc(name = "set_state_data_by_id")]
    fn set_state_by_id(
        &self,
        state_id: u8,
        state: StateData,
        token: String,
    ) -> BoxFuture<RPCResult<Result<(), Errors>>>;

    #[rpc(name = "read_file_by_path")]
    fn read_file_by_path(
        &self,
        path: String,
        filesystem_name: String,
        state_id: u8,
        token: String,
    ) -> BoxFuture<RPCResult<Result<FileInfo, Errors>>>;

    #[rpc(name = "write_file_by_path")]
    fn write_file_by_path(
        &self,
        path: String,
        content: String,
        filesystem_name: String,
        state_id: u8,
        token: String,
    ) -> BoxFuture<RPCResult<Result<(), Errors>>>;

    #[rpc(name = "list_dir_by_path")]
    fn list_dir_by_path(
        &self,
        path: String,
        filesystem_name: String,
        state_id: u8,
        token: String,
    ) -> BoxFuture<RPCResult<Result<Vec<DirItemInfo>, Errors>>>;

    #[rpc(name = "get_ext_info_by_id")]
    fn get_ext_info_by_id(
        &self,
        extension_id: String,
        state_id: u8,
        token: String,
    ) -> BoxFuture<RPCResult<Result<ManifestInfo, Errors>>>;

    #[rpc(name = "get_ext_list_by_id")]
    fn get_ext_list_by_id(
        &self,
        state_id: u8,
        token: String,
    ) -> BoxFuture<RPCResult<Result<Vec<String>, Errors>>>;
}

/// JSON RPC manager
pub struct RpcManager {
    pub states: Arc<Mutex<StatesList>>,
}

/// Implementation of all JSON RPC methods
impl RpcMethods for RpcManager {
    /// Return the state by the given ID if found
    fn get_state_by_id(
        &self,
        state_id: u8,
        token: String,
    ) -> BoxFuture<RPCResult<Option<StateData>>> {
        let states = self.states.clone();
        Box::pin(async move {
            let states = states.lock().await;
            // Try to get the requested state
            if let Some(state) = states.get_state_by_id(state_id) {
                let state = state.lock().await;
                // Make sure the token is valid
                if state.has_token(&token) {
                    Ok(Some(state.data.clone()))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        })
    }

    /// Update an state
    fn set_state_by_id(
        &self,
        state_id: u8,
        new_state_data: StateData,
        token: String,
    ) -> BoxFuture<RPCResult<Result<(), Errors>>> {
        let states = self.states.clone();
        Box::pin(async move {
            let states = states.lock().await;
            // Try to get the requested state
            if let Some(state) = states.get_state_by_id(state_id) {
                let mut state = state.lock().await;
                // Make sure the token is valid
                if state.has_token(&token) {
                    tracing::info!("Updated state by id <{}>", state.data.id);
                    state.update(new_state_data).await;
                    Ok(Ok(()))
                } else {
                    Ok(Err(Errors::BadToken))
                }
            } else {
                Ok(Err(Errors::StateNotFound))
            }
        })
    }

    /// Returns the content of a file
    /// Internally implemented by the given filesystem
    fn read_file_by_path(
        &self,
        path: String,
        filesystem_name: String,
        state_id: u8,
        token: String,
    ) -> BoxFuture<RPCResult<Result<FileInfo, Errors>>> {
        let states = self.states.clone();
        Box::pin(async move {
            let states = states.lock().await;
            // Try to get the requested state
            if let Some(state) = states.get_state_by_id(state_id) {
                let state = state.lock().await;
                // Make sure the token is valid
                if state.has_token(&token) {
                    // Try to get the requested filesystem implementation
                    if let Some(filesystem) = state.get_fs_by_name(&filesystem_name) {
                        let filesystem = filesystem.lock().await;
                        let result = filesystem.read_file_by_path(&path);
                        let result = result.await;

                        state.notify_extensions(ExtensionMessages::ReadFile(
                            state_id,
                            filesystem_name,
                            result.clone(),
                        ));
                        Ok(result)
                    } else {
                        Ok(Err(Errors::Fs(FilesystemErrors::FilesystemNotFound)))
                    }
                } else {
                    Ok(Err(Errors::BadToken))
                }
            } else {
                Ok(Err(Errors::StateNotFound))
            }
        })
    }

    /// Writes new content to the specified path
    fn write_file_by_path(
        &self,
        path: String,
        content: String,
        filesystem_name: String,
        state_id: u8,
        token: String,
    ) -> BoxFuture<RPCResult<Result<(), Errors>>> {
        let states = self.states.clone();
        Box::pin(async move {
            let states = states.lock().await;
            // Try to get the requested state
            if let Some(state) = states.get_state_by_id(state_id) {
                let state = state.lock().await;
                // Make sure the token is valid
                if state.has_token(&token) {
                    // Try to get the requested filesystem implementation
                    if let Some(filesystem) = state.get_fs_by_name(&filesystem_name) {
                        let filesystem = filesystem.lock().await;
                        let result = filesystem.write_file_by_path(&path, &content);
                        let result = result.await;

                        state.notify_extensions(ExtensionMessages::WriteFile(
                            state_id,
                            filesystem_name,
                            content,
                            result.clone(),
                        ));
                        Ok(result)
                    } else {
                        Ok(Err(Errors::Fs(FilesystemErrors::FilesystemNotFound)))
                    }
                } else {
                    Ok(Err(Errors::BadToken))
                }
            } else {
                Ok(Err(Errors::StateNotFound))
            }
        })
    }

    /// Returns the list of items inside the given directory
    /// Internally implemented by the given filesystem
    fn list_dir_by_path(
        &self,
        path: String,
        filesystem_name: String,
        state_id: u8,
        token: String,
    ) -> BoxFuture<RPCResult<Result<Vec<DirItemInfo>, Errors>>> {
        let states = self.states.clone();
        Box::pin(async move {
            let states = states.lock().await;
            // Try to get the requested state
            if let Some(state) = states.get_state_by_id(state_id) {
                let state = state.lock().await;
                // Make sure the token is valid
                if state.has_token(&token) {
                    // Try to get the requested filesystem implementation
                    if let Some(filesystem) = state.get_fs_by_name(&filesystem_name) {
                        let filesystem = filesystem.lock().await;
                        let result = filesystem.list_dir_by_path(&path);
                        let result = result.await;

                        state.notify_extensions(ExtensionMessages::ListDir(
                            state_id,
                            filesystem_name,
                            path,
                            result.clone(),
                        ));

                        Ok(result)
                    } else {
                        Ok(Err(Errors::Fs(FilesystemErrors::FilesystemNotFound)))
                    }
                } else {
                    Ok(Err(Errors::BadToken))
                }
            } else {
                Ok(Err(Errors::StateNotFound))
            }
        })
    }

    /// Returns the information about a extension
    fn get_ext_info_by_id(
        &self,
        extension_id: String,
        state_id: u8,
        token: String,
    ) -> BoxFuture<RPCResult<Result<ManifestInfo, Errors>>> {
        let states = self.states.clone();
        Box::pin(async move {
            let states = states.lock().await;
            // Try to get the requested state
            if let Some(state) = states.get_state_by_id(state_id) {
                let state = state.lock().await;
                // Make sure the token is valid
                if state.has_token(&token) {
                    // Try to get the requested info about the extension
                    Ok(state.get_ext_info_by_id(&extension_id))
                } else {
                    Ok(Err(Errors::BadToken))
                }
            } else {
                Ok(Err(Errors::StateNotFound))
            }
        })
    }
    /// Returns the list of extensions in the specified state
    fn get_ext_list_by_id(
        &self,
        state_id: u8,
        token: String,
    ) -> BoxFuture<RPCResult<Result<Vec<String>, Errors>>> {
        let states = self.states.clone();
        Box::pin(async move {
            let states = states.lock().await;
            // Try to get the requested state
            if let Some(state) = states.get_state_by_id(state_id) {
                let state = state.lock().await;
                // Make sure the token is valid
                if state.has_token(&token) {
                    // Try to get the requested info about the extension
                    Ok(Ok(state.get_ext_list_by_id()))
                } else {
                    Ok(Err(Errors::BadToken))
                }
            } else {
                Ok(Err(Errors::StateNotFound))
            }
        })
    }
}
