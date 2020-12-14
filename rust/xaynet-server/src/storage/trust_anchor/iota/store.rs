use redis::{
    aio::ConnectionManager,
    AsyncCommands,
    FromRedisValue,
    IntoConnectionInfo,
    RedisError,
    RedisResult,
    RedisWrite,
    ToRedisArgs,
    Value,
};
use tracing::debug;

use super::client::AuthorState;
use crate::storage::coordinator_storage::redis::impls::redis_type_error;

const KEY_AUTHOR_STATE: &str = "author_state";

#[derive(Clone)]
pub struct AuthorStore {
    connection: ConnectionManager,
}

impl AuthorStore {
    pub async fn new<T: IntoConnectionInfo>(url: T) -> Result<Self, RedisError> {
        let client = redis::Client::open(url)?;
        let connection = client.get_tokio_connection_manager().await?;
        Ok(Self { connection })
    }

    pub async fn author_state(&mut self) -> RedisResult<Option<AuthorState>> {
        // https://redis.io/commands/get
        // > Get the value of key. If the key does not exist the special value nil is returned.
        //   An error is returned if the value stored at key is not a string, because GET only
        //   handles string values.
        // > Return value
        //   Bulk string reply: the value of key, or nil when key does not exist.
        debug!("get author state");
        self.connection.get(KEY_AUTHOR_STATE).await
    }

    pub async fn set_author_state(&mut self, state: &AuthorState) -> RedisResult<()> {
        // https://redis.io/commands/set
        // > Set key to hold the string value. If key already holds a value,
        //   it is overwritten, regardless of its type.
        // Possible return value in our case:
        // > Simple string reply: OK if SET was executed correctly.
        debug!("set author state");
        self.connection.set(KEY_AUTHOR_STATE, state).await?;
        Ok(())
    }
}

impl FromRedisValue for AuthorState {
    fn from_redis_value(v: &Value) -> RedisResult<AuthorState> {
        match *v {
            Value::Data(ref bytes) => bincode::deserialize(bytes)
                .map_err(|e| redis_type_error("Invalid data", Some(e.to_string()))),
            _ => Err(redis_type_error("Response not bincode compatible", None)),
        }
    }
}

impl<'a> ToRedisArgs for &'a AuthorState {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + RedisWrite,
    {
        // safe to unwrap:
        // - all the sequences have known length
        // - no untagged enum
        let data = bincode::serialize(self).unwrap();
        data.write_redis_args(out)
    }
}

#[cfg(test)]
impl AuthorStore {
    /// Deletes all data in the current database.
    pub async fn flush_db(&mut self) -> RedisResult<()> {
        debug!("flush current database");
        // https://redis.io/commands/flushdb
        // > This command never fails.
        redis::cmd("FLUSHDB")
            .arg("ASYNC")
            .query_async(&mut self.connection)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iota_streams::app_channels::api::tangle::Address;
    use serial_test::serial;

    async fn init_store() -> AuthorStore {
        let mut store = AuthorStore::new("redis://127.0.0.1/").await.unwrap();
        store.flush_db().await.unwrap();
        store
    }

    #[tokio::test]
    #[serial]
    async fn integration_set_and_get_author_state() {
        // test the writing and reading of the author state
        let mut store = init_store().await;
        let address = Address::from_str(
            "0add45b7b8502218c2d6b8236696d468a4a34bcb03eecea8ed097e153f16fd500000000000000000",
            "9e7d04aa0d1f79dfc04a3b3d",
        )
        .unwrap();

        let set_state = AuthorState::new(vec![1, 2, 3], &address);
        store.set_author_state(&set_state).await.unwrap();

        let get_state = store.author_state().await.unwrap().unwrap();

        assert_eq!(set_state, get_state)
    }

    #[tokio::test]
    #[serial]
    async fn integration_get_author_state_empty() {
        // test the reading of a non existing author state
        let mut store = init_store().await;

        let get_state = store.author_state().await.unwrap();

        assert_eq!(None, get_state)
    }
}
