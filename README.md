# COSC473 Assignment Two SwapBytes: `decent-share`

`decent-share` is a decentralised file swapping application. `decent-share`
provides a global chat for all connected peers, direct messaging to a single
peer, and file exchanging functionality. Files must be shared in a one for one
swapping manor, where to receive someone else's file you must also offer them
a file of your own.

## Usage

To run `decent-share`, execute its binary file on the command line:

```bash
./decent-share
```

`decent-share` will connect to peers on the same local network using mDNS and
listen to `stdin` for actions to perform. There are six different actions one
can perform within `decent-share`.

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
parameters: trade this \<file> found at \<this path> for this \<person>'s
\<file> and place it at this \<path>.

```sh
trade <offered_file_name> <path_to_source_offered_file> <recipient_username> <requested_file_name> <path_to_place_requested_file>
```

The recipient of the trade can then respond to this trade using either of the
`accept` or `decline` actions.

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
