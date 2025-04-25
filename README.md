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

To use `decent-share`, it is critical that the rendezvous server is running.
When a node is booted up, it connects to the rendezvous server in order to
discover other peers and be discoverable to other peers. Run the rendezvous
server by executing its binary. Optionally set the value of the `RUST_LOG`
environment variable to enable logging to stdout.

```bash
./rendezvous_server
RUST_LOG=info ./rendezvous_server
```

## Starting a new peer

Once the rendezvous server is running, you may start up a client node. To do so,
execute the binary file on the command line. `decent-share` takes a single
command line argument, `--rendezvous-address`/`-r`, to specify the IPv4 address
that the rendezvous server is hosted on. This must be input in dotted decimal
notation. If you are running a node on the same machine as the rendezvous
server, you may omit this argument as it defaults to `127.0.0.1`.

```bash
./decent-share --rendezvous-address 198.162.0.1
```

`decent-share` will connect to peers who have also registered on the rendezvous
server. The first node to be booted will emit a warning that there are no known
peers. Without any other peers connected, none of `decent-share`'s features will
function.

## Usage

Once another peer has connected, `decent-share` will listen to `stdin` for
actions to perform. There are six different actions one can perform.

* register
* send
* dm
* trade
* accept
* decline

The first action one must perform is `register`. This gives you a username so
that other peers can trade with and direct message you. To register a name,
type the following, replacing `<username>` with the name you would like to
register as.

```sh
register <username>
```

Once you are registered, you can introduce yourself to other users with `send`.
Messages sent using the `send` action are broadcast to all active users of
`decent-share`. Here you can tell peers what files you have to offer and ask
others what they have available.

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
