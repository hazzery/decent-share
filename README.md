# COSC473 Assignment Two SwapBytes: `decent-share`

`decent-share` is a decentralised file swapping application. `decent-share`
provides a global chat for all connected peers, direct messaging to a single
peer, and file exchanging functionality. Files must be shared in a one for one
swapping manor, where to receive someone else's file you must also offer them
a file of your own.

## Building from source

To build `decent-share` (and the rendezvous server), run the following in the
project directory.

```bash
cargo build
```

## Booting the rendezvous server

`decent-share` can be used across networks through the use of a rendezvous
server, or over a local access network using mDNS. If inter-network
communication is not required, there is no need for the rendezvous server to be
running, peers will automatically find each other when running on the same
network. If two peers are not connected to the same network, it is critical that
the rendezvous server is running.

When an IP address is specified for a rendezvous server, new nodes will
immediately connect to the rendezvous server in order to discover other peers
and be discoverable to other peers.

To run the rendezvous server, execute its binary on the command line. Optionally
set the value of the `RUST_LOG` environment variable to enable logging to
stdout.

```bash
./rendezvous_server
RUST_LOG=info ./rendezvous_server
```

## Starting a new peer

To boot a new node, execute the binary file on the command line. `decent-share`
takes up to two arguments. `--username`/`-u` must be specified, this is the name
other users will see when you send messages and trade offers. The second
argument, `--rendezvous-address`/`-r`, is optional. If specified, it must be
an IPv4 address (in dotted decimal notation), which a rendezvous server is
listening on. The rendezvous server's port number is fixed, so this should not
be added. As described above if you do not wish to communicate with peers
outside your local network, this argument can be left unspecified.

```bash
./decent-share --username name --rendezvous-address 198.162.0.1
```

## Usage

Once your node has made a connection to another node, `decent-share` will emit
a message that your username has successfully been registered on the network.
This will be almost instant when connection through mDNS but may take a few
seconds when connecting to a rendezvous server. It will then listen to `stdin`
for actions to perform. There are six different actions one can perform.

* send
* dm
* trade
* accept
* decline

To send a chat message, you can use `send`. Chat messages sent using the `send`
action are broadcast to all active users of `decent-share`. Here you can tell
peers what files you have to offer and ask others what they have available.

```sh
send <message>
```

To communicate private information about a particular trade with a single user,
the `dm` action can come in handy. Direct messages are sent directly to their
recipient, they are not propagated through the network.

```sh
dm <recipient> <message>
```

Now we are ready to offer a trade to a peer, we will need the `trade` action.
When using `trade`, remember the following to help with usage of the action's
parameters: trade this \<file> (found at \<path>) for \<username>'s \<file> and
place it at \<path>.

```sh
trade <offered_file_name> <path_to_source_offered_file> <recipient_username> <requested_file_name> <path_to_place_requested_file>
```

The recipient of the trade can then respond to this trade using either of the
`accept` or `decline` actions.

When using `accept`, remember the following to help with usage of the action's
parameters: accept \<username>'s offer of \<file>, (which should be placed at
\<path>) for my \<file> (which can be found at \<path>).

Similarly, when using `decline`, remember the following: decline \<username>'s
offer of \<file> for my \<file>.

```sh
accept <offerer_username> <offered_file_name> <path_to_place_offered_file> <requested_file_name> <path_to_source_requested_file>
decline <offerer_username> <offered_file_name> <requested_file_name>
```

## Example

Bob:

```sh
register bob
```

Alice:

```sh
register alice
```

Bob:

```sh
trade bobs_473_tutorial_notes ~/documents/COSC473/tutorial_one.rs alice alices_401_lecture_notes ~/documents/COSC401/lecture_one.md
```

Alice:

```sh
accept bob bobs_473_tutorial_notes ~/Documents/university/473_tutorial_one.rs alices_401_lecture_notes ~/Documents/university/401_lecture_one.md
```

After Bob and Alice have executed the preceding actions,
`bobs_473_tutorial_notes` will be copied from
`~/documents/COSC473/tutorial_one.rs` on Bob's machine to
`~/Documents/university/473_tutorial_one.rs` on Alice's machine and
`alices_401_lecture_notes` will be copied from
`~/Documents/university/401_lecture_one.md` on Alice's machine to
`~/documents/COSC401/lecture_one.md` on Bob's machine.
