# Layered Molecular Editor (LME)

This is a program for automatically constructing molecular models and prepare for first-principle computations by delaretive-programming input file and resources organized by it.

## Installation

### Software requirements

LME is written in Rust and should be easily to compile with Rust toolchains, we will also provide binary files of release versions in the GitHub release. 

Besides the LME itself, you still need to install follow software:

- OpenBabel 3.X: OpenBabel will help this program to generate output files in standard format.
- GNU sed (Optional): GNU sed allows users to custom the output file with regular expression in the input file.

> Please add the installation directory to PATH environment variable

> You may find LME can now work without OpenBabel, but in later version it will become a strong dependency.

### Hardware requirements

- CPU: The LME itself doesn't contain any platform-specified code, but as most first-principle calculation software works only on AMD64 platform, we only test it on AMD64 CPUs. There is no minimum CPU performace requirements and number of cores is more important than frequency for LME itself, but the user-developed plugins may require better single-core performance.
- Memory usage: The runtime memory is mainly used to store the layer index of each model and information about recently built and used structures cached based on the LRU algorithm, the former usually increases with the number of models and modelling steps, while the latter can be controlled in terms of the number of reservations using the `LME_CACHE_SIZE` environment variable. In most tasks, the peak running memory will not exceed 2 GB.
- Hard disk: The layers are stored in a embedded database on the hard disk, which usally takes less than 1GB space. Though the total amount of data is small, the embedded database will wait the file system to synchronise the write operations to disk, so the SSDs can significantly improve the performance.

### Installation

In the release package, serveral binary files are provided. You can put them in anywhere you like, and add the directory to PATH environment variable.

The binary files should contain:

- lmers: The main program of LME.
- obabelme: Tools convert other molecular files between common format to LME format.    

### Other resources

- `snippets` folder contains some bash or Python code snippets which might be useful to construct program integrate with LME.

## Concepts

### Layers

In this program, each molecular structure is described by a stack of the "modeling layers", which means molecular structures derived from the same template will share the same underlying stack. This mechanism saves storage space and preserve the correspondence between models when a large number of derived molecules are constructed.

### Runners
