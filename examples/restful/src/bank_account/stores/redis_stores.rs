use cqrs_es2::Error;

use cqrs_es2_store::{
    redis_store::{
        EventStore,
        QueryStore,
    },
    Repository,
};

use crate::cqrs::db_connection;

use super::super::{
    aggregate::BankAccount,
    commands::BankAccountCommand,
    dispatchers::LoggingDispatcher,
    events::BankAccountEvent,
    queries::BankAccountQuery,
};

type ThisEventStore =
    EventStore<BankAccountCommand, BankAccountEvent, BankAccount>;

type ThisQueryStore = QueryStore<
    BankAccountCommand,
    BankAccountEvent,
    BankAccount,
    BankAccountQuery,
>;

type ThisRepository = Repository<
    BankAccountCommand,
    BankAccountEvent,
    BankAccount,
    ThisEventStore,
>;

pub fn get_event_store() -> Result<ThisRepository, Error> {
    Ok(ThisRepository::new(
        ThisEventStore::new(db_connection().unwrap()),
        vec![
            Box::new(get_query_store().unwrap()),
            Box::new(LoggingDispatcher::new()),
        ],
        true,
    ))
}

pub fn get_query_store() -> Result<ThisQueryStore, Error> {
    Ok(ThisQueryStore::new(
        db_connection().unwrap(),
    ))
}
