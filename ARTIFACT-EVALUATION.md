# Artifact Appendix

Paper title: **Sybil-Resistant Parallel Mixnets**

Artifacts HotCRP Id: **#13**

Requested Badge: **Reproduced**

## Description
This repository implements a configurable fully connected parallel mixnet which can support either Sybil resistant or non-Sybil resistant communication.

It uses BBS+ signatures as a fork of the BBS+ crate and includes zero knowledge proofs to achieve Sybil resistance.

## Basic Requirements (Only for Functional and Reproduced badges)

### Hardware Requirements
The hardware resources required to launch the mixnet correlate to the desired number of mix nodes and packets.
To run a minimal mixnet with 2 mix nodes and 1000 clients any PC would do and it should run in a matter of seconds and storage should be less then 10 MB. 
To reproduce the results of the paper, i.e. using 80 nodes with 2 million clients, 4 CPU's and 50G of memory per node would be required.

### Software Requirements
All necessary software packages are listed in the `cargo.toml` file, python3 is also required. 

- **Locally:** To run locally using the python script `run.py` a Windows OS is required.
- **Remotely:** To run remotely while using the scripts under `./scripts/` a `x86_64-unknown-linux-gnu` architecture and a Slurm cluster are required.

### Estimated Time and Storage Consumption

#### Mixnet Setup Stage
The setup time is linear to the amount of generated traffic. 
Assuming 64 CPU's and 50G of memory generating the longest evaluation setup with 2 million simulated clients takes about 2 hours whereas generating 100k will take a couple of minutes.

In total to reproduce all results the setup stages would take around 45 hours combined.

#### Launching the Mixnet
Launching the mixnet as evaluated in the paper with the following configuration:
- 5 layers
- 3 circuit setup rounds
- 3 communication rounds 
and with 4 CPU's, 50G memory per mix node will take up to 1 hour for 2 million clients and a couple of minutes for 100k. 

In total to reproduce all results the mixnet running stages would take at most 24 hours combined.

## Environment 

### Accessibility (All badges)
The following git repository tags contain the source code, build and launch instructions under the `README.md` for this artifact:

- Github Repository: https://github.com/maya-kleinstein/Sybil-Resistant-Mixnet
- Tags: artifact, yodel_artifact

Note: artifact is the relevant tag unless explicitly stated otherwise.

### Set up the environment (Only for Functional and Reproduced badges)

#### Required Directory Structure
Before running the setup stage or launching the mixnet, ensure the following directory structure exists:

```
./data/
├── info/
│   └── config_info
├── ips/
└── logs/
```

Each subdirectory serves a specific purpose:

- `info/` – Stores mixnet metadata or heavy pre-generated information.
- `ips/` – Holds mix IP's for syncing communication (NOTE: this must be deleted manually in case mixnet stopped mid-run)
- `logs/` – Output logs for debugging and monitoring.
- `config_info` - Stores a configuration for the mixnet.

An example for a configuration in `config_info` could be:
```
{"base_port":8000,"num_mixes":2,"num_clients":1000,"percentage_bad_clients":0.0,"num_layers":5,"first_measured_layer":1,"mix_verification":"Verify","num_setup_rounds":3,"num_data_rounds":3,"data_size":128,"is_proof_compressed":true,"edge_limit":1.1}
```

Where:
- `base_port`: `base_port + i` is the port the i'th mix will communicate - `mix_verification`: is the verification mode used to generate and decrypt packets and can be either `Verify` or `NoVerification` meaning whether proofs will exist and be verified in circuit setup packets. 
- `percentage_bad_clients`: The number of malicious clients who all target a singular mix node per layer.
- `first_measured_layer`, `is_proof_compressed` and `edge_limit` are all set to their default values and shouldn't be changed.
- The rest of the values are straight forward.

**NOTICE:** The directories should all be empty except for `./data/info` and any final log files under `./data/logs/`. If the mixnet crashes or stops mid run they must be cleaned manually before the next launch by running:
```
rm ./data/ips/*
find ./data/logs -maxdepth 1 -type f ! -name 'log*' -exec rm {} +
```
#### Mixnet Setup Stage
To avoid heavy computation during runtime all of the encryption keys and encrypted circuit setup and data packets are pre-generated during the setup stage. 

This is done using the `setup` binary and requires no parameters except for the above configuration.

Notice that there will be no clients directly communicating with the launched mixnet - instead the mix servers will get the pre-generated files from `./data/info` and once decrypted will send them to the `configurator` binary that verifies it has received all expected packets.

### Testing the Environment (Only for Functional and Reproduced badges)
To simultaneously run and check the mixnet setup stage you can run the `marshalling_test` under `./tests/mixnet.rs` using:

```bash
cargo test --release marshalling_test  -- --nocapture
```

This is only recommended for short setups since running on debug mode can negatively impact runtime.

## Artifact Evaluation (Only for Functional and Reproduced badges)

### Main Results and Claims

#### Main Result 1: Micro-Benchmarks
The paper shows micro-benchmarks that describe the overhead of registering a client, decrypting and verifying their circuit setup packets.
The micro-benchmarks are shown in section 6.1 and can all be calculated using experiment #1. 

#### Main Result 2: System Performance
The system performance results show the expected latency of the launched mixnet for various configurations of the mixnet.

The results are shown in section 6.2 and can be calculated using experiment #2.

### Experiments 
List each experiment the reviewer has to execute. Describe:
 - How to execute it in detailed steps.
 - What the expected result is.
 - How long it takes and how much space it consumes on disk. (approximately)
 - Which claim and results does it support, and how.

#### Experiment 1: Micro-Benchmarks
Run the following to reproduce the micro-benchmarks described below from section 6.1:
```bash
cargo bench
```
-  Credential Issuance time: Is the result of running the `register_client` benchmark
- Public Key Decryption: Is the result of running the `decrypt_setup_packet_layer` benchmark
- Ticket Validation: Is the result of running the `verify_proof` benchmark

This takes a couple of minutes requires <10MB of disk space.

#### Experiment 2: System Performance
To launch the mixnet first the environment needs to be set up as described above

##### Locally
To run the experiment locally on a windows machine (mostly for verifying functionality) you can build using cargo and run the `run.py` script as follows:
```bash
python3 run.py _NUM_MIXES_ local _IF_TO_SETUP_
```
where `_NUM_MIXES_` is the number of mix nodes as described in `config_info` and `_IF_TO_SETUP_` is whether to generate pre-computed information before the launch or to use what's already been generated.

Alternatively you could use the test `system_test` - this DOES NOT launch seperate processes and instead launches everything in a multithreaded fashion:

```bash
cargo test --release system_test  -- --nocapture
```

Notice that local runs are mostly useful for testing functionality and debugging and don't necessarily reflect the perfomance on a remote cluster.

##### Remotely
To run on a remote cluster managed by SLURM, you can compile using the Dockefile to build an image and run a container by running:
```bash
docker build -t $IMAGE_NAME
docker run -d --name $CONTAINER_NAME -v $LOCAL_CODE_DIR:/code $IMAGE_NAME bash -c "while true; do sleep 10; done"
```

Then using the following you can produce all the necessary binaries:
```bash
docker start $CONTAINER_NAME
docker exec $CONTAINER_NAME bash -c "cargo build --release --target x86_64-unknown-linux-gnu"
docker stop $CONTAINER_NAME
```

On the cluster itself you must first setup the required environment, first by setting up the directories and configuration file as described earlier and then by running the `run_setup.py` script in `./scripts/`.
I recommend using this SBATCH configuration for setup:
```bash
#!/bin/bash

#SBATCH --ntasks-per-node=1
#SBATCH --cpus-per-task=64
#SBATCH --nodes=1
#SBATCH --time=04:00:00
#SBATCH --mem=50G

echo "Starting Mixnet\n"

srun -n 1 python3 run_setup.py
```

Once the setup is done you can launch the mixnet using the number of mixes you configurated in the `./data/config` file, for the papers experiments that would be 80:
```bash
#!/bin/bash

#SBATCH --ntasks-per-node=8
#SBATCH --cpus-per-task=4
#SBATCH --nodes=10
#SBATCH --time=01:00:00
#SBATCH --mem=50G


#SBATCH hetjob
#SBATCH --ntasks-per-node=1
#SBATCH --cpus-per-task=4
#SBATCH --nodes=1
#SBATCH --time=01:00:00
#SBATCH --mem=50G


echo "Starting Mixnet\n"

srun -n 80 python3 run_mix.py : -n 1 python3 run_config.py remote
```

You can adjust the SBATCH values as desired.

To reproduce the papers results experiments with all possible configuration combinations must be run:
- `num_clients`: `(100k, 500k, 1M, 2M)`
- `percentage_bad_clients`: `(0.0, 0.25, 0.5)`
- `mix_verification`: `(Verify, NoVerification)`

All other values should be set as described in the environment setup example.

#### Yodel Comparison: 

To reproduce the yodel experiments the above must be done again but this time using the `yodel_artifact` tag in the same repo on the `yodel_mixnet` branch.

## Limitations (Only for Functional and Reproduced badges)
The accessibility to a slurm cluster is a limitation to reproducibility.

Assuming all the basic requirements described in the section above all of the results presented in the paper are reproducible given this artifact.

## Notes on Reusability (Only for Functional and Reproduced badges)
This artifact could be used as a framework for developing Sybil-Resistant parallel mixnets with additional features, such as:
- adding support for non-fully connected mixnets
- adding batch verification to the circuit setup proof verification to improve efficiency
- Implementing other compatible mixnets such as Yodel on top of the current framework
- allowing for real-time client connections and communication, and not only via pre-computed packets.