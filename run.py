import argparse
import subprocess
import time

# Parse flags (num servers, num clients, etc.)
parser = argparse.ArgumentParser(description='Start remote test.')
parser.add_argument('mixes', type=int,
                    help='number of servers')
parser.add_argument('remote', type=str,
                    help='running on remote ips or local ips')
parser.add_argument('setup', type=bool,
                    help="Choose whether to setup the config and network files or not")

args = parser.parse_args()

bin_path = "C:\\Thesis\\Sybil-Resistant-Mixnet\\target\\release"

# Setup files
if args.setup:
    print("Setting up all files")
    cmd = "{}\\setup.exe".format(bin_path)
    setup_p = subprocess.Popen(cmd, stdout=None, stderr=None, stdin=subprocess.PIPE, shell=True)


# Launch the Mixes
print("launching mixes")
mprocesses = []
for i in range(args.mixes):
    cmd = "{}\\mix.exe {} {}".format(bin_path, args.remote, i)
    p = subprocess.Popen(cmd, stdout=None, stderr=None, stdin=subprocess.PIPE, shell=True)
    mprocesses.append(p)
# time.sleep(0.1)


# Launch the Configurator
print("launching configurator")
cmd = "{}\\config.exe {}".format(bin_path, args.remote)
config_p = subprocess.Popen(cmd, stdout=None, stderr=None, stdin=subprocess.PIPE, shell=True)
# time.sleep(0.5)


# Cleanup
# print("cleanup processes")
# for p in mprocesses:
#     os.kill(p.pid, signal.SIGINT)
# os.kill(config_p.pid, signal.SIGINT)
