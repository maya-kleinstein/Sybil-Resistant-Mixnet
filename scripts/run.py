import argparse
import os
import signal
import subprocess
import time

# Parse flags (num servers, num clients, etc.)
parser = argparse.ArgumentParser(description='Start remote test.')
parser.add_argument('mixes', type=int,
                    help='number of servers')
parser.add_argument('clients', type=int,
                    help='number of clients')
parser.add_argument('remote', type=bool,
                    help='running on remote ips')
parser.add_argument('kill', type=int,
                    help='kill all process first [0|1]')

args = parser.parse_args()
kill = args.kill == 1


# Get IP's
ips = []

if args.remote:
    with open('remote_ips') as f:
        lines = f.readlines()
        for line in lines:
            ips.append(line.rstrip('\n'))
else:
    ips = ['localhost' for _ in range(args.mixes + args.clients)]

mix_ips = []
client_ips = []
for i in range(args.mixes):
    mix_ips.append((ips[i],8000))
for i in range(args.clients):
    client_ips.append((ips[i],10000))

bin_path = "target/debug"

# Launch the Mixes
print("launching mixes")
mprocesses = []
for i in range(args.mixes):
    cmd = "C:\university\Thesis\bbs\target\debug\mix.exe"
    # TODO: cmd won't work ofc...fix later
    if args.remote:
        cmd = " ".join(['ssh',
                    mix_ips[i][0],
                    '%s/mix' % bin_path])
    p = subprocess.Popen(cmd, stdout=None, stderr=None, stdin=subprocess.PIPE, shell=True)
    mprocesses.append(p)
time.sleep(0.5)


# Launch the Clients (write IP+port to predetermined files)
print("launching clients")
cprocesses = []
for i in range(args.clients):
    cmd = "C:\university\Thesis\bbs\target\debug\client.exe"
    # TODO: cmd won't work ofc...fix later
    if args.remote:
        cmd = " ".join(['ssh',
                    client_ips[i][0],
                    '%s/client' % bin_path])
    p = subprocess.Popen(cmd, stdout=None, stderr=None, stdin=subprocess.PIPE, shell=True)
    cprocesses.append(p)
time.sleep(0.5)


# Launch the Configurator
print("launching configurator")
cmd = "C:\university\Thesis\bbs\target\debug\configurator.exe"
# TODO: cmd won't work ofc...fix later
if args.remote:
    cmd = '%s/config' % bin_path
config_p = subprocess.Popen(cmd, stdout=None, stderr=None, stdin=subprocess.PIPE, shell=True)
time.sleep(0.5)


# Cleanup
print("cleanup processes")
for p in cprocesses:
    os.kill(p.pid, signal.SIGINT)
for p in mprocesses:
    os.kill(p.pid, signal.SIGINT)
os.kill(config_p.pid, signal.SIGINT)
