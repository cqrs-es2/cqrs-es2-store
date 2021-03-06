use log::{
    debug,
    trace,
};
use std::marker::PhantomData;

use rusqlite::{
    params,
    Connection,
};

use cqrs_es2::{
    Error,
    EventContext,
    IAggregate,
    ICommand,
    IEvent,
    IQuery,
    QueryContext,
};

use crate::repository::{
    IEventDispatcher,
    IQueryStore,
};

use super::super::mysql_constants::*;

static CREATE_QUERY_TABLE: &str = "
CREATE TABLE IF NOT EXISTS
queries
(
    aggregate_type TEXT                        NOT NULL,
    aggregate_id   TEXT                        NOT NULL,
    query_type     TEXT                        NOT NULL,
    version        bigint CHECK (version >= 0) NOT NULL,
    payload        TEXT                        NOT NULL,
    PRIMARY KEY (aggregate_type, aggregate_id, query_type)
);
";

/// SQLite storage
pub struct QueryStore<
    C: ICommand,
    E: IEvent,
    A: IAggregate<C, E>,
    Q: IQuery<C, E>,
> {
    conn: Connection,
    _phantom: PhantomData<(C, E, A, Q)>,
}

impl<
        C: ICommand,
        E: IEvent,
        A: IAggregate<C, E>,
        Q: IQuery<C, E>,
    > QueryStore<C, E, A, Q>
{
    /// Constructor
    pub fn new(conn: Connection) -> Self {
        Self {
            conn,
            _phantom: PhantomData,
        }
    }

    fn create_query_table(&mut self) -> Result<(), Error> {
        match self
            .conn
            .execute(CREATE_QUERY_TABLE, [])
        {
            Ok(_) => {},
            Err(e) => {
                return Err(Error::new(
                    format!(
                        "unable to create queries table with error: \
                         {}",
                        e
                    )
                    .as_str(),
                ));
            },
        };

        debug!("Created queries table");

        Ok(())
    }
}

impl<
        C: ICommand,
        E: IEvent,
        A: IAggregate<C, E>,
        Q: IQuery<C, E>,
    > IQueryStore<C, E, A, Q> for QueryStore<C, E, A, Q>
{
    /// saves the updated query
    fn save_query(
        &mut self,
        context: QueryContext<C, E, Q>,
    ) -> Result<(), Error> {
        self.create_query_table()?;

        let aggregate_type = A::aggregate_type();
        let query_type = Q::query_type();

        let aggregate_id = context.aggregate_id;

        debug!(
            "storing a new query for aggregate id '{}'",
            &aggregate_id
        );

        let sql = match context.version {
            1 => INSERT_QUERY,
            _ => UPDATE_QUERY,
        };

        let payload = match serde_json::to_string(&context.payload) {
            Ok(x) => x,
            Err(e) => {
                return Err(Error::new(
                    format!(
                        "unable to serialize the payload of query \
                         '{}' with aggregate id '{}', error: {}",
                        &query_type, &aggregate_id, e,
                    )
                    .as_str(),
                ));
            },
        };

        match self.conn.execute(
            sql,
            params![
                context.version,
                payload,
                aggregate_type,
                aggregate_id,
                query_type,
            ],
        ) {
            Ok(x) => x,
            Err(e) => {
                return Err(Error::new(
                    format!(
                        "unable to insert/update query for \
                         aggregate id '{}' with error: {}",
                        &aggregate_id, e
                    )
                    .as_str(),
                ));
            },
        };

        Ok(())
    }

    /// loads the most recent query
    fn load_query(
        &mut self,
        aggregate_id: &str,
    ) -> Result<QueryContext<C, E, Q>, Error> {
        self.create_query_table()?;

        let aggregate_type = A::aggregate_type();
        let query_type = Q::query_type();

        trace!(
            "loading query '{}' for aggregate id '{}'",
            query_type,
            aggregate_id
        );

        let mut sql = match self.conn.prepare(SELECT_QUERY) {
            Ok(x) => x,
            Err(e) => {
                return Err(Error::new(
                    format!(
                        "unable to prepare queries table for query \
                         '{}' with aggregate id '{}', error: {}",
                        &query_type, &aggregate_id, e,
                    )
                    .as_str(),
                ));
            },
        };

        let res = match sql.query_map(
            params![aggregate_type, aggregate_id, query_type],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ) {
            Ok(x) => x,
            Err(e) => {
                return Err(Error::new(
                    format!(
                        "unable to load queries table for query \
                         '{}' with aggregate id '{}', error: {}",
                        &query_type, &aggregate_id, e,
                    )
                    .as_str(),
                ));
            },
        };

        let mut rows: Vec<(i64, String)> = Vec::new();

        for x in res {
            rows.push(x.unwrap());
        }

        if rows.len() == 0 {
            trace!(
                "returning default query '{}' for aggregate id '{}'",
                query_type,
                aggregate_id
            );

            return Ok(QueryContext::new(
                aggregate_id.to_string(),
                0,
                Default::default(),
            ));
        }

        let row = rows[0].clone();

        let payload = match serde_json::from_str(row.1.as_str()) {
            Ok(x) => x,
            Err(e) => {
                return Err(Error::new(
                    format!(
                        "bad payload found in queries table for \
                         query '{}' with aggregate id '{}', error: \
                         {}",
                        &query_type, &aggregate_id, e,
                    )
                    .as_str(),
                ));
            },
        };

        Ok(QueryContext::new(
            aggregate_id.to_string(),
            row.0,
            payload,
        ))
    }
}

impl<
        C: ICommand,
        E: IEvent,
        A: IAggregate<C, E>,
        Q: IQuery<C, E>,
    > IEventDispatcher<C, E> for QueryStore<C, E, A, Q>
{
    fn dispatch(
        &mut self,
        aggregate_id: &str,
        events: &Vec<EventContext<C, E>>,
    ) -> Result<(), Error> {
        self.dispatch_events(aggregate_id, events)
    }
}
