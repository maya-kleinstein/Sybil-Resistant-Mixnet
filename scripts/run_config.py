# NOTE: this should ONLY work on the cluster.

import argparse
import subprocess
import time

# Parse flags (num servers, num clients, etc.)
parser = argparse.ArgumentParser(description='Start remote test.')
parser.add_argument('remote', type=str,
                    help='running on remote ips or local ips')
parser.add_argument('setup', type=bool,
                    help="Choose whether to setup the config and network files or not")

args = parser.parse_args()

bin_path = "/cs/labs/yossigi/maya_k/Sybil_Resistant_Mixnet/target/x86_64-unknown-linux-gnu/release"

# Setup files
if args.setup:
    print("Setting up all files")
    cmd = "{}/setup".format(bin_path)
    setup_p = subprocess.Popen(cmd, stdin=subprocess.PIPE, shell=True)

# Launch the Configurator
print("launching configurator")
cmd = "{}/config {}".format(bin_path, args.remote)
config_p = subprocess.Popen(cmd, stdin=subprocess.PIPE, shell=True)
# time.sleep(0.5)

# Cleanup
# print("cleanup processes")
# os.kill(config_p.pid, signal.SIGINT)