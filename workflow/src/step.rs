use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::{
    runner::{Runner, RunnerOutput},
    workflow_data::WorkflowData,
};

#[derive(Debug, Deserialize)]
pub struct Step {
    from: Option<String>,
    name: Option<String>,
    run: Runner,
}

impl Step {
    pub fn execute(self, index: usize, workflow_data: &mut WorkflowData) -> Result<()> {
        if let Some(from) = self.from {
            let window = workflow_data
                .windows
                .get(&from)
                .cloned()
                .with_context(|| format!("Failed to load window with name {}", from))?;
            workflow_data.current_window = window;
        }
        let generated_stacks = self.run.execute(
            &workflow_data.base,
            &workflow_data.current_window,
            &mut workflow_data.layers.borrow_mut(),
        )?;
        match generated_stacks {
            RunnerOutput::SingleWindow(generated_stacks) => {
                workflow_data.current_window = generated_stacks;
            }
            RunnerOutput::MultiWindow(named_stacks) => {
                let prefix = self.name.clone().unwrap_or(index.to_string());
                workflow_data.current_window = BTreeMap::new();
                for (suffix, generated_stacks) in named_stacks {
                    workflow_data
                        .current_window
                        .extend(generated_stacks.clone());
                    let name = [prefix.to_string(), suffix].join("_");
                    workflow_data.windows.insert(name, generated_stacks);
                }
            }
            RunnerOutput::None => {}
        };
        if let Some(name) = self.name {
            if workflow_data
                .windows
                .insert(name.to_string(), workflow_data.current_window.clone())
                .is_some()
            {
                println!("Over take window named {}", name);
            }
        }
        Ok(())
    }
}
