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
        let current_window = workflow_data.current_window_stacks()?;
        let generated_stacks = self.run.execute(
            &workflow_data.base,
            current_window,
            &mut workflow_data.layers.borrow_mut(),
        )?;
        let start = workflow_data.stacks.len();
        match generated_stacks {
            RunnerOutput::Serial(generated_stacks) => {
                workflow_data.stacks.extend(generated_stacks);
                workflow_data.current_window = start..workflow_data.stacks.len();
            }
            RunnerOutput::Named(named_stacks) => {
                let prefix = self.name.clone().unwrap_or(index.to_string());
                for (suffix, genenrated_stacks) in named_stacks {
                    let name = [prefix.to_string(), suffix].join("_");
                    let start = workflow_data.stacks.len();
                    workflow_data.stacks.extend(genenrated_stacks);
                    workflow_data
                        .windows
                        .insert(name, start..workflow_data.stacks.len());
                }
                workflow_data.current_window = start..workflow_data.stacks.len();
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
