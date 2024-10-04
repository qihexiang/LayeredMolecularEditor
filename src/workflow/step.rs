use std::sync::{Arc, RwLock};

use lme::workspace::StackCache;
use serde::Deserialize;

use crate::{error::WorkflowError, runner::Runner, workflow_data::WorkflowData};

#[derive(Deserialize)]
pub struct Step {
    from: Option<String>,
    name: Option<String>,
    run: Runner,
}

impl Step {
    pub fn execute(
        &self,
        workflow_data: &mut WorkflowData,
        cache: Arc<RwLock<StackCache>>,
    ) -> Result<(), WorkflowError> {
        if let Some(from) = &self.from {
            let window = workflow_data
                .windows
                .get(from)
                .cloned()
                .ok_or(WorkflowError::WindowNotFound(from.clone()))?;
            workflow_data.current_window = window;
        }
        let next_window_start = workflow_data.workspace.stacks.len();
        let additional_named_windows = self.run.execute(workflow_data, cache)?;
        let next_window_stop = workflow_data.workspace.stacks.len();
        let next_window = next_window_start..next_window_stop;
        if let Some(name) = &self.name {
            if workflow_data
                .windows
                .insert(name.clone(), next_window.clone())
                .is_some()
            {
                println!("Operation window named {} is replaced.", name);
            }

            if let Some(additional_named_windows) = additional_named_windows {
                workflow_data.windows.extend(
                    additional_named_windows
                        .into_iter()
                        .map(|(add_name, window)| ([name, add_name.as_str()].join("_"), window)),
                );
            }
        }

        workflow_data.current_window = next_window;
        Ok(())
    }
}
