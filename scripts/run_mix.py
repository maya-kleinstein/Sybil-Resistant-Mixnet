# NOTE: this should ONLY work on the cluster.

import subprocess
import time

bin_path = "./Sybil_Resistant_Mixnet/target/x86_64-unknown-linux-gnu/release"

# Launch the Mixes
print("launching mix")
cmd = "{}/mix remote".format(bin_path)
p = subprocess.Popen(cmd, stdin=subprocess.PIPE, shell=True)