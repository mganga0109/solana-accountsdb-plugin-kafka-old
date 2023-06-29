// Copyright 2022 Blockdaemon Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use {
    crate::{
        prom::{
            StatsThreadedProducerContext, UPLOAD_ACCOUNTS_TOTAL, UPLOAD_SLOTS_TOTAL,
            UPLOAD_TRANSACTIONS_TOTAL,
        },
        *,
    },
    prost::Message,
    rdkafka::{
        error::KafkaError,
        producer::{BaseRecord, Producer, ThreadedProducer},
    },
    std::time::Duration,
    bs58,
};

pub struct Publisher {
    producer: ThreadedProducer<StatsThreadedProducerContext>,
    shutdown_timeout: Duration,

    update_account_topic: String,
    slot_status_topic: String,
    transaction_topic: String,
    publish_separate_program: bool,
}

impl Publisher {
    pub fn new(producer: ThreadedProducer<StatsThreadedProducerContext>, config: &Config) -> Self {
        Self {
            producer,
            shutdown_timeout: Duration::from_millis(config.shutdown_timeout_ms),
            update_account_topic: config.update_account_topic.clone(),
            slot_status_topic: config.slot_status_topic.clone(),
            transaction_topic: config.transaction_topic.clone(),
            publish_separate_program: config.publish_separate_program.clone(),
        }
    }

    pub fn update_account(&self, ev: UpdateAccountEvent) -> Result<(), KafkaError> {
        let mut topic_with_suffix = format!("{}", self.update_account_topic);

        if !self.publish_separate_program {
            let pubkey_base58 = bs58::encode(&ev.owner).into_string();
            topic_with_suffix = format!("{}-{}", self.update_account_topic, pubkey_base58);
        }

        let buf = ev.encode_to_vec();
        let record = BaseRecord::<Vec<u8>, _>::to(&topic_with_suffix)
            .key(&ev.pubkey)
            .payload(&buf);
        let result = self.producer.send(record).map(|_| ()).map_err(|(e, _)| e);
        UPLOAD_ACCOUNTS_TOTAL
            .with_label_values(&[if result.is_ok() { "success" } else { "failed" }])
            .inc();
        result
    }

    pub fn update_slot_status(&self, ev: SlotStatusEvent) -> Result<(), KafkaError> {
        let buf = ev.encode_to_vec();
        let record = BaseRecord::<(), _>::to(&self.slot_status_topic).payload(&buf);
        let result = self.producer.send(record).map(|_| ()).map_err(|(e, _)| e);
        UPLOAD_SLOTS_TOTAL
            .with_label_values(&[if result.is_ok() { "success" } else { "failed" }])
            .inc();
        result
    }

    pub fn update_transaction(&self, ev: TransactionEvent) -> Result<(), KafkaError> {
        let buf = ev.encode_to_vec();
        let record = BaseRecord::<(), _>::to(&self.transaction_topic).payload(&buf);
        let result = self.producer.send(record).map(|_| ()).map_err(|(e, _)| e);
        UPLOAD_TRANSACTIONS_TOTAL
            .with_label_values(&[if result.is_ok() { "success" } else { "failed" }])
            .inc();
        result
    }

    pub fn wants_update_account(&self) -> bool {
        !self.update_account_topic.is_empty()
    }

    pub fn wants_slot_status(&self) -> bool {
        !self.slot_status_topic.is_empty()
    }

    pub fn wants_transaction(&self) -> bool {
        !self.transaction_topic.is_empty()
    }
}

impl Drop for Publisher {
    fn drop(&mut self) {
        self.producer.flush(self.shutdown_timeout);
    }
}
