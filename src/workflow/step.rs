use std::{collections::BTreeMap, env::current_dir, fs::File, io::Read};

use anyhow::{anyhow, Context, Result};
use lazy_static::lazy_static;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use fancy_regex::Regex;
use serde::Deserialize;
use url::Url;

use super::{
    runner::{cached_read_stack, Runner, RunnerOutput},
    workflow_data::WorkflowData,
};

#[derive(Debug, Deserialize)]
pub struct Step {
    pub from: Option<String>,
    pub name: Option<String>,
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
            &workflow_data.layers,
        )?;

        let cache_generated_stacks = |generated_stacks: &BTreeMap<String, Vec<u64>>| {
            generated_stacks
            .par_iter()
            .map(|(_, stack_path)| {
                cached_read_stack(
                    &workflow_data.base,
                    &workflow_data.layers,
                    &stack_path,
                )
            })
            .collect::<Result<Vec<_>, _>>()
        };

        // Cache for generated stacks before result commit to current window, avoid lazy-computation defer the error.
        match generated_stacks {
            RunnerOutput::SingleWindow(generated_stacks) => {
                cache_generated_stacks(&generated_stacks)?;
                workflow_data.current_window = generated_stacks;
            }
            RunnerOutput::MultiWindow(named_stacks) => {
                let prefix = self.name.clone().unwrap_or(index.to_string());
                workflow_data.current_window = BTreeMap::new();
                for (suffix, generated_stacks) in named_stacks {
                    cache_generated_stacks(&generated_stacks)?;
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

#[derive(Debug, Deserialize, Default)]
#[serde(try_from = "StepsLoader")]
pub struct Steps(pub Vec<Step>);

impl Steps {
    fn concat(mut a: Self, mut b: Self) -> Self {
        let mut steps = vec![];
        steps.append(&mut a.0);
        steps.append(&mut b.0);
        Self(steps)
    }

    fn push(&mut self, value: Step) {
        self.0.push(value);
    }
}

#[derive(Deserialize, Debug)]
struct StepsLoader(Vec<StepLoader>);

impl TryFrom<StepsLoader> for Steps {
    type Error = anyhow::Error;

    fn try_from(value: StepsLoader) -> Result<Self> {
        let mut inner = vec![];
        for loader in value.0 {
            let Steps(result) = Steps::try_from(loader)?;
            inner.extend(result);
        }
        Ok(Steps(inner))
    }
}

#[derive(Deserialize, Debug)]
struct StepLoader {
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    run: Option<Runner>,
    #[serde(default)]
    load: Option<String>,
}

lazy_static! {
    static ref YAML_NULLABLE_VARIABLE_RE: Regex = Regex::new(r"\{\{ __.* \}\}").unwrap();
}

/// Generate step list from input file.
///
/// The `run` field specify the first step in the loader, if no `run` field specified, the CheckPoint runner will be used.
/// The `from` field will be always attached to the first step.
///
/// The `load` field speicifies steps loaded from other files, which would be put after the first step. if no `loader` specified,
/// the `name` field will be attached to the first step, otherwise a CheckPoint step will be automatically created at the end of
/// the step queue and the `name` field will be attached to it.
///
impl TryFrom<StepLoader> for Steps {
    type Error = anyhow::Error;
    fn try_from(value: StepLoader) -> Result<Self> {
        let mut steps = Steps(vec![Step {
            from: value.from,
            name: if value.load.is_none() {
                value.name.clone()
            } else {
                None
            },
            run: value.run.unwrap_or_default(),
        }]);

        if let Some(filepath) = value.load {
            let url = if filepath.starts_with("/") {
                Url::parse(&format!("file:{}", filepath))?
            } else {
                let url = Url::from_directory_path(current_dir()?)
                    .map_err(|_| anyhow!("Unable to get current working direcotry"))?;
                url.join(&filepath)?
            };
            let filepath = url
                .to_file_path()
                .map_err(|_| anyhow!("Unable to convert URL {} to filepath", url))?;
            if filepath
                .file_stem()
                .with_context(|| anyhow!("Filename with no file stem is not allowed now"))?
                .to_string_lossy()
                .to_string()
                .ends_with("template")
            {
                println!(
                    "Loading template {:?} with query string: {:?}",
                    filepath,
                    url.query()
                );
                let mut file = File::open(&filepath)
                    .with_context(|| format!("Failed to open target file {:?}", filepath))?;
                let mut content = String::new();
                file.read_to_string(&mut content)
                    .with_context(|| anyhow!("Failed to read file {:?}", &filepath))?;
                for (k, v) in url.query_pairs() {
                    let k = format!("{{{{ {} }}}}", k);
                    content = content.replace(&k, &v);
                }
                let content = YAML_NULLABLE_VARIABLE_RE.replace_all(&content, "null");
                println!("Input from template generated: \n{}", content);
                steps = Steps::concat(steps, serde_yaml::from_str(&content)?);
            } else {
                println!("Loading {:?}", filepath);
                let file = File::open(&filepath)
                    .with_context(|| format!("Failed to open target file {:?}", filepath))?;
                steps = Steps::concat(steps, serde_yaml::from_reader(file)?);
            }
            if value.name.is_some() {
                steps.push(Step {
                    from: None,
                    name: value.name,
                    run: Runner::default(),
                });
            }
        };

        Ok(steps)
    }
}
