# Sybil-Resistant-Mixnet
This repo implements a Sybil Resistant Fully Connected Parallel Mixnet using BBS+ signatures as a fork of the BBS+ crate.

## How can I Run it?
To run it anywhere, first a configuration must be setup in ./data/info/config_info
An example for such a configuration could be:
```
{"base_port":8000,"num_mixes":2,"num_clients":1000,"percentage_bad_clients":0.0,"num_layers":5,"first_measured_layer":1,"mix_verification":"OnlyVerifyEdgeCases","num_setup_rounds":3,"num_data_rounds":3,"data_size":128,"is_proof_compressed":true,"edge_limit":1.1}
```
This configuration will be used by the setup binary to pre-generate all necessary information such as: keys, data/setup packets, etc.

Notice that the mixnet will be launched by running all the mix servers as well as a "configurator" that will act as the receiver of all the final onion decrypted packets.

### Locally
To run locally on a windows machine you can build and run the run.py script as follows:
```
python run.py NUM_MIXES local IF_TO_SETUP
```
Notice that local runs are mostly useful for debugging and don't necessarily reflect the perfomance on a remote cluster.

### Remotely
To run on a remote cluster managed by SLURM, you can compile on a windows machine using the Dockefile and the setup.sh script.
On the cluster itself you must then run the run_setup.py script first, I recommend using this SBATCH configuration for setup:
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

