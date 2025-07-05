import argparse
import subprocess
import os
import platform

# Parse flags (num servers, num clients, etc.)
parser = argparse.ArgumentParser(description='Start remote test.')
parser.add_argument('mixes', type=int,
                    help='number of servers')
parser.add_argument('remote', type=str,
                    help='running on remote ips or local ips')
parser.add_argument('setup', type=bool,
                    help="Choose whether to setup the config and network files or not")

args = parser.parse_args()

# Detect platform and define executable extension and separator
is_windows = platform.system() == "Windows"
exe_suffix = ".exe" if is_windows else ""
bin_path = os.path.join("target", "release")

def full_path(binary_name):
    return os.path.join(bin_path, binary_name + exe_suffix)

def run_process(cmd_list):
    return subprocess.Popen(cmd_list, stdout=None, stderr=None, stdin=subprocess.PIPE)

# Setup files
if args.setup:
    print("Setting up all files")
    setup_cmd = [full_path("setup")]
    setup_p = run_process(setup_cmd)
    stdout, stderr = setup_p.communicate()

# Launch Mixes
print("Launching mixes")
mprocesses = []
for i in range(args.mixes):
    mix_cmd = [full_path("mix"), args.remote, str(i)]
    p = run_process(mix_cmd)
    mprocesses.append(p)

# Launch Configurator
print("Launching configurator")
config_cmd = [full_path("config"), args.remote]
config_p = run_process(config_cmd)

# Cleanup
print("Waiting for processes to finish")
for p in mprocesses:
    p.communicate()
config_p.communicate()
