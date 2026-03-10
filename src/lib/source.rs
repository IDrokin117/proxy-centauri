use crate::registry::Limits;
use anyhow::{anyhow, Result};
use csv::Reader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::Mutex;
use tracing::info;

pub(crate) enum DBConnection {
    Csv(CSVConnection),
    Sql(SQLConnection),
}

impl Connection for DBConnection {
    type Record = UserRecord;

    async fn establish(&self) -> Result<()> {
        match self {
            DBConnection::Csv(conn) => conn.establish().await,
            DBConnection::Sql(conn) => conn.establish().await,
        }
    }

    async fn fetch(&self, username: &str) -> Result<Option<UserRecord>> {
        match self {
            DBConnection::Csv(conn) => conn.fetch(username).await,
            DBConnection::Sql(conn) => conn.fetch(username).await,
        }
    }

    async fn close(&self) -> Result<()> {
        match self {
            DBConnection::Csv(conn) => conn.close().await,
            DBConnection::Sql(conn) => conn.close().await,
        }
    }
}

trait Connection {
    type Record;
    async fn establish(&self) -> Result<()>;
    async fn fetch(&self, username: &str) -> Result<Option<Self::Record>>;

    async fn close(&self) -> Result<()>;
}

pub(crate) struct CSVConnectionParameters {
    file_path: PathBuf,
}
impl CSVConnectionParameters {
    pub(crate) fn new(path_buf: PathBuf) -> Self {
        CSVConnectionParameters {
            file_path: path_buf,
        }
    }
}
impl Default for CSVConnectionParameters {
    fn default() -> Self {
        CSVConnectionParameters::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("files/db.csv"))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct UserRecord {
    username: String,
    password: String,
    proxy_username: Option<String>,
    proxy_password: Option<String>,
    concurrency_limit: Option<u16>,
    traffic_limit: Option<u128>,
    status: String,
}

impl UserRecord {
    pub(crate) fn is_authenticated(&self, password: &str) -> bool {
        self.password == password
    }
}

impl From<UserRecord> for Limits {
    fn from(value: UserRecord) -> Self {
        Limits::new(value.concurrency_limit, value.traffic_limit)
    }
}

pub(crate) enum ConnectionState {
    Uninit,
    Opened,
    Closed,
}
pub(crate) struct CSVConnection {
    params: CSVConnectionParameters,
    data: Mutex<Vec<UserRecord>>,
    state: Mutex<ConnectionState>,
}

impl CSVConnection {
    pub(crate) fn new(params: CSVConnectionParameters) -> Self {
        CSVConnection {
            params,
            data: Mutex::new(Vec::new()),
            state: Mutex::new(ConnectionState::Uninit),
        }
    }
}

impl Connection for CSVConnection {
    type Record = UserRecord;

    async fn establish(&self) -> Result<()> {
        const MAX_FILE_SIZE: u64 = 10_000;
        if std::fs::metadata(&self.params.file_path).map(|m| m.len())? > MAX_FILE_SIZE {
            return Err(anyhow!(format!(
                "CSV file must be less than {MAX_FILE_SIZE} bytes"
            )));
        }
        let mut state = self.state.lock().await;
        match *state {
            ConnectionState::Uninit => {
                let mut reader = Reader::from_path(&self.params.file_path)?;
                let mut data: Vec<UserRecord> = Vec::new();

                for record in reader.deserialize() {
                    let user: UserRecord = record?;
                    data.push(user);
                }
                *self.data.lock().await = data;
                *state = ConnectionState::Opened;
                Ok(())
            }
            ConnectionState::Opened => {
                info!("Connection already opened");
                Ok(())
            }
            ConnectionState::Closed => Err(anyhow!("Can not reopen closed connection")),
        }
    }

    async fn fetch(&self, username: &str) -> Result<Option<UserRecord>> {
        Ok(self
            .data
            .lock()
            .await
            .iter()
            .find(|el| el.username == username)
            .cloned())
    }

    async fn close(&self) -> Result<()> {
        self.data.lock().await.clear();
        *self.state.lock().await = ConnectionState::Closed;
        Ok(())
    }
}

pub(crate) struct SQLConnection {}

impl Connection for SQLConnection {
    type Record = UserRecord;

    async fn establish(&self) -> Result<()> {
        todo!("SQL connection not implemented")
    }

    async fn fetch(&self, _username: &str) -> Result<Option<UserRecord>> {
        todo!()
    }
    async fn close(&self) -> Result<()> {
        todo!()
    }
}

pub(crate) struct Backend {
    connection: DBConnection,
    cache: Mutex<HashMap<String, UserRecord>>,
}

impl Backend {
    pub(crate) fn new(connection: DBConnection) -> Self {
        Self {
            connection,
            cache: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) async fn fetch_user(&self, username: &str) -> Result<Option<UserRecord>> {
        let mut guard = self.cache.lock().await;
        if guard.contains_key(username) {
            return Ok(guard.get(username).cloned());
        }
        self.connection.establish().await?;
        if let Some(user) = self.connection.fetch(username).await? {
            guard.insert(username.to_string(), user);
        }
        Ok(guard.get(username).cloned())
    }
}
