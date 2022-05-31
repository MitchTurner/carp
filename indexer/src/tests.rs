use std::collections::VecDeque;
use std::sync::Mutex;
use oura::model::{BlockRecord, Era, Event, EventData};
use oura::sources::PointArg;
use entity::sea_orm::MockDatabaseConnector;
use crate::postgres_sink::{Config, InputReceiver};
use super::*;

struct MockReceiver {
    events: Mutex<VecDeque<Event>>
}

impl MockReceiver {
    pub fn new() -> Self {
        MockReceiver {
            events: Default::default()
        }
    }

    pub fn with_event_data(&self, data: EventData) {
        let event = Event {
            context: Default::default(),
            data,
            fingerprint: None
        };
        self.events.lock().unwrap().push_front(event)
    }
}

impl InputReceiver for MockReceiver {
    fn recv(&self) -> anyhow::Result<Event> {
        if let Some(event) = self.events.lock().unwrap().pop_back() {
            Ok(event)
        } else {
            // TODO: Do we want to block here?
            Err(anyhow!("Out of events :("))
        }
    }
}

fn some_block_record() -> BlockRecord {
    BlockRecord {
        era: Era::Alonzo,
        epoch: None,
        epoch_slot: None,
        body_size: 0,
        issuer_vkey: "".to_string(),
        tx_count: 0,
        slot: 0,
        hash: "".to_string(),
        number: 0,
        previous_hash: "".to_string(),
        cbor_hex: Some(String::new()),
        transactions: None
    }
}

#[tokio::test]
async fn start__sanity_check() {
    let postgres_url = "postgresql://carp:password@localhost:5432/carp_mainnet";
    let conn = MockDatabaseConnector::connect(&postgres_url).await.unwrap();
    let input = MockReceiver::new();
    input.with_event_data(EventData::Block(some_block_record()));

    let initial_point = Some(PointArg(0,"".to_string()));
    let exec_plan = Arc::new(ExecutionPlan(Default::default()));

    let sink_setup = Config::new(&conn);

    let result = sink_setup.start(input, exec_plan, initial_point.as_ref()).await;

    dbg!(result);
}
