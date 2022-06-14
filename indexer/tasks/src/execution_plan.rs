use std::collections::HashMap;
use std::fs;

use toml::Value;
use tracing_subscriber::prelude::*;

#[derive(Debug)]
pub struct ExecutionPlan(pub toml::value::Table);

impl ExecutionPlan {
    pub fn load_from_file(path: &str) -> ExecutionPlan {
        match &fs::read_to_string(path) {
            Ok(execution_plan_content) => {
                let setting: Result<toml::value::Table, toml::de::Error> =
                    toml::from_str(execution_plan_content);

                ExecutionPlan(setting.unwrap())
            }
            Err(err) => {
                tracing::error!("No execution plan found at {}", path);
                panic!("{}", err);
            }
        }
    }
}

impl From<Vec<(&str, HashMap<String, bool>)>> for ExecutionPlan {
    fn from(tasks: Vec<(&str, HashMap<String, bool>)>) -> Self {
        let map = tasks
            .into_iter()
            .map(|(task, hmap)| (task.to_string(), hashmap_into_table(&hmap)))
            .collect();
        ExecutionPlan(map)
    }
}

fn hashmap_into_table(hmap: &HashMap<String, bool>) -> Value {
    let map = hmap
        .iter()
        .map(|(key, value)| (key.to_string(), Value::Boolean(*value)))
        .collect();
    Value::Table(map)
}
