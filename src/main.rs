mod workflow;

use std::{
    fs::File,
    path::PathBuf,
};

use anyhow::Context;
use workflow::{
    input_data::{WorkflowCheckPoint, WorkflowInput},
    workflow_data::{LayerStorage, WorkflowData},
};

fn main() {
    let input: WorkflowInput = serde_yaml::from_reader(
        File::open("lme_workflow.inp.yaml")
            .with_context(|| "Failed to open lme_workflow.inp.yaml in current directory")
            .unwrap(),
    )
    .unwrap();

    set_path(input.binaries).unwrap();

    let checkpoint = load_checkpoint();
    let (skip, mut workflow_data) = if let Some(WorkflowCheckPoint {
        skip,
        base,
        layers,
        windows,
        current_window,
    }) = checkpoint
    {
        (
            skip,
            WorkflowData {
                base,
                layers:LayerStorage::try_from(layers).unwrap(),
                windows,
                current_window,
            },
        )
    } else {
        (0, WorkflowData::new(input.base, input.layer_storage.unwrap_or(PathBuf::from(".layer_storage.db"))))
    };

    let mut checkpoint = WorkflowCheckPoint {
        skip,
        base: workflow_data.base.clone(),
        layers: workflow_data.layers.get_config(),
        windows: workflow_data.windows.clone(),
        current_window: workflow_data.current_window.clone(),
    };

    let mut last_step_name = String::from("start");
    let mut last_step_index = 0;

    for (index, step) in input.steps.0.into_iter().enumerate().skip(skip) {
        if let Some(name) = &step.name {
            last_step_name = name.to_string();
            last_step_index = index;
        }
        println!(
            "Enter step: {}, {} steps after {}, steps after window size: {}",
            index,
            index - last_step_index,
            last_step_name,
            workflow_data.current_window.len()
        );
        match step.execute(index, &mut workflow_data) {
            Ok(_) => {
                if !input.no_checkpoint {
                    checkpoint = WorkflowCheckPoint {
                        skip: index + 1,
                        base: workflow_data.base.clone(),
                        layers: workflow_data.layers.get_config(),
                        windows: workflow_data.windows.clone(),
                        current_window: workflow_data.current_window.clone(),
                    };
                }
            }
            Err(err) => {
                if !input.no_checkpoint {
                    println!("Error. Saving checkpoint file");
                    dump_checkpoint(&checkpoint);
                }
                panic!("{:#?}", err)
            }
        }
    }

    if !input.no_checkpoint {
        println!("Finished. Saving checkpoint file");
        dump_checkpoint(&checkpoint);
    }

    println!("finished");
}

fn load_checkpoint() -> Option<WorkflowCheckPoint> {
    let workflow_data_file = File::open("lme_workflow.chk.data").ok()?;
    Some(
        serde_json::from_reader(
            zstd::Decoder::new(workflow_data_file)
                .with_context(|| "Failed to create zstd decompress pipe")
                .unwrap(),
        )
        .with_context(|| "Unable to deserialize lme_workflow.chk.data, it might be broken")
        .unwrap(),
    )
}

fn set_path(user_specified_paths: Vec<PathBuf>) -> anyhow::Result<()> {
    let current_binary_directory = PathBuf::from(
        std::env::current_exe()?
            .parent()
            .expect("Binary file must have a parent directory"),
    );
    let working_directory_bin = std::env::current_dir()?.join("bin");
    let current_path_var = std::env::var_os("PATH").unwrap_or_default();
    let current_path_var = std::env::split_paths(&current_path_var);
    let mut paths = user_specified_paths;
    paths.extend([working_directory_bin, current_binary_directory]);
    paths.extend(current_path_var);
    let paths = std::env::join_paths(paths)?;
    std::env::set_var("PATH", paths);
    Ok(())
}

fn dump_checkpoint(checkpoint: &WorkflowCheckPoint) {
    serde_json::to_writer(
        zstd::Encoder::new(
            File::create("lme_workflow.chk.data")
                .with_context(|| "Unable to create lme_workflow.chk.data")
                .unwrap(),
            9,
        )
        .with_context(|| "Unable to create zstd compress pipe")
        .unwrap()
        .auto_finish(),
        checkpoint,
    )
    .with_context(|| "Unable to serialize lme_workflow.chk.data")
    .unwrap()
}
