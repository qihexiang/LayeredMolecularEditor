use std::{
    fs::File,
    sync::{Arc, RwLock},
};

use input_data::{WorkflowCheckPoint, WorkflowInput};
use lme::workspace::StackCache;
use workflow_data::WorkflowData;

mod error;
mod input_data;
mod runner;
mod step;
mod workflow_data;

fn main() {
    let input: WorkflowInput =
        serde_yaml::from_reader(File::open("lme_workflow.inp.yaml").unwrap()).unwrap();
    let check_point: Option<WorkflowCheckPoint> = File::open("lme_workflow.chk.yaml")
        .ok()
        .and_then(|file| serde_yaml::from_reader(file).ok());
    let (steps, mut workflow_data) = if let Some(check_point) = check_point {
        let workflow_data = check_point.workflow_data;
        let steps = input.steps.into_iter().skip(check_point.skip).collect();
        println!("Workflow data loaded from checkpoint file.");
        (steps, workflow_data)
    } else {
        let workflow_data = WorkflowData::new(input.base);
        let steps = input.steps;
        println!("Workflow data created from input file.");
        (steps, workflow_data)
    };
    let cache: Arc<RwLock<StackCache>> = Default::default();
    for (index, step) in steps.into_iter().enumerate() {
        step.execute(&mut workflow_data, cache.clone()).unwrap();
        let checkpoint = WorkflowCheckPoint {
            skip: index + 1,
            workflow_data: workflow_data.clone(),
        };
        let file = File::create("lme_workflow.chk.yaml").unwrap();
        serde_yaml::to_writer(file, &checkpoint).unwrap();
    }
    println!("finished")
}
