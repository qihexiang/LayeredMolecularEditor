use std::fs::File;

use error::WorkflowError;
use input_data::{WorkflowCheckPoint, WorkflowInput};
use workflow_data::WorkflowData;
use zstd::{Decoder, Encoder};

mod error;
mod input_data;
mod runner;
mod step;
mod workflow_data;

fn main() {
    let input: WorkflowInput = serde_yaml::from_reader(
        File::open("lme_workflow.inp.yaml")
            .map_err(|_| WorkflowError::InputFileNotFound)
            .unwrap(),
    )
    .unwrap();

    let check_point: Option<WorkflowCheckPoint> = File::open("lme_workflow.chk.yaml.zstd")
        .ok()
        .and_then(|file| serde_yaml::from_reader(Decoder::new(file).unwrap()).ok());
    let (skiped, steps, mut workflow_data) = if let Some(check_point) = check_point {
        let workflow_data = check_point.workflow_data;
        let steps = input.steps.into_iter().skip(check_point.skip).collect();
        println!("Workflow data loaded from checkpoint file.");
        (check_point.skip, steps, workflow_data)
    } else {
        let workflow_data = WorkflowData::new(input.base);
        let steps = input.steps;
        println!("Workflow data created from input file.");
        (0, steps, workflow_data)
    };

    let mut checkpoint = WorkflowCheckPoint {
        skip: skiped,
        workflow_data: workflow_data.clone(),
    };

    for (index, step) in steps.into_iter().enumerate() {
        println!(
            "Enter step: {}, window size: {}",
            index + skiped,
            workflow_data.current_window.len()
        );
        match step.execute(index, &mut workflow_data) {
            Ok(_) => {
                if !input.no_checkpoint {
                    checkpoint = WorkflowCheckPoint {
                        skip: skiped + index + 1,
                        workflow_data: workflow_data.clone(),
                    };
                }
            }
            Err(err) => {
                if !input.no_checkpoint {
                    println!("Error. Saving checkpoint file");
                    let file = File::create("lme_workflow.chk.yaml.zstd").unwrap();
                    let zstd_encoder = Encoder::new(file, 9).unwrap().auto_finish();
                    serde_yaml::to_writer(zstd_encoder, &checkpoint).unwrap();
                }
                panic!("{:#?}", err)
            }
        }
    }

    if !input.no_checkpoint {
        println!("Finished. Saving checkpoint file");
        let file = File::create("lme_workflow.chk.yaml.zstd").unwrap();
        let zstd_encoder = Encoder::new(file, 9).unwrap().auto_finish();
        serde_yaml::to_writer(zstd_encoder, &checkpoint).unwrap();
    }

    println!("finished");
}
