# Sybil-Resistant-Mixnet
This repo implements a Sybil Resistant Fully Connected Parallel Mixnet as described in the PETS 2025 paper **Sybil-Resistant Parallel Mixnets**

It uses BBS+ signatures as a fork of the BBS+ crate and includes extended zero knowledge proofs as described in the paper.

## How can I Launch the Mixnet?

### Setup

#### Required Directory Structure

Before launching the mixnet, ensure the following directory structure exists:

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

Where `base_port + i` is the port the i'th mix will communicate from, `mix_verification` can be `Verify` or `NoVerification` meaning whether proofs will exist and be verified in circuit setup packets. The rest of the values are straight forward or contain their default values. 

#### Generating Pre-computed Data
To avoid heavy computation during runtime all of the encryption keys and encrypted circuit setup and data packets are pre-generated during the setup stage. 

This is done using the `setup` binary and requires no parameters except for the above configuration.

Notice that there will be no clients directly communicating with the launched mixnet - instead the mix servers will get the pre-generated files from `./data/info` and once decrypted will send them to the `configurator` binary that verifies it has received all expected packets.

### Locally
To run locally on a windows machine you can build using cargo and run the run.py script as follows:
```
python3 run.py _NUM_MIXES_ local _IF_TO_SETUP_
```

Alternatively you could use the tests `test_marshalling` (that acts like the setup binary) and then `test_system` - this DOES NOT launch seperate processes and instead launches everything in a multithreaded fashion.

Notice that local runs are mostly useful for debugging and don't necessarily reflect the perfomance on a remote cluster.

### Remotely
To run on a remote cluster managed by SLURM, you can compile using the Dockefile to build an image and run a container, then using the following you can produce all the necessary binaries:

```
docker exec $CONTAINER_NAME bash -c "cargo build --release --target x86_64-unknown-linux-gnu"
```

On the cluster itself you must then run the `run_setup.py` script first, I recommend using this SBATCH configuration for setup:
```
#!/bin/bash

#SBATCH --ntasks-per-node=1
#SBATCH --cpus-per-task=64
#SBATCH --nodes=1
#SBATCH --time=04:00:00
#SBATCH --mem=50G

echo "Starting Mixnet\n"

srun -n 1 python3 run_setup.py
```

Once the setup is done you can launch the mixnet using any number of mixes you configurated in the ./data/config file, for example - for 80 that would look like:
```
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

## How can I Benchmark the System?
### Microbenchmarks
To run microbenchmarks you can use `cargo bench`, this produces benchmarks for registration, data/setup packet decryption as well as ticket verification.

### System Benchmarks
To produce system-wide benchmarks you can run the mixnet remotely as described above with the configuration you wish to benchmark. The logs should contain how long each round took each mix to run for a fully connected mixnet.

