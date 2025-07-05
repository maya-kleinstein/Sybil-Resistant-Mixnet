# NOTE: this should ONLY work on the cluster.

import argparse
import subprocess
import time

# Parse flags (num servers, num clients, etc.)
parser = argparse.ArgumentParser(description='Start remote test.')
parser.add_argument('remote', type=str,
                    help='running on remote ips or local ips')

args = parser.parse_args()

bin_path = "./target/x86_64-unknown-linux-gnu/release"

# Launch the Configurator
time.sleep(5)
print("launching configurator")
cmd = "{}/config {}".format(bin_path, args.remote)
config_p = subprocess.Popen(cmd, stdin=subprocess.PIPE, shell=True)