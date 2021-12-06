# Specification

This specification is split into two parts. The first one explaining how each type is represented in binary form and the
second one specifying the behaviour of event passing and handling.

## Binary Representation of Events

Events store contain two types of data. The header data with the id, name, namespace and referenced event id and the
payload with the associated data. Both the header and the payload have a dynamic length and should be extendable while
ensuring backwards compatibility. The binary layout looks like this:

| Name          | Type      | Length (bytes)             | Description                        |
|---------------|-----------|----------------------------|------------------------------------|
| total_length  | `u64`     | 8                          | total length of the event in bytes |
| header_length | `u16`     | 2                          | length of the event header         |
| header        | `Header`  | header_length              | the header of the event            |
| data          | `Vec<u8>` | total_length - data_length | the payload of the event           |

### Header

| Name             | Type     | Length (bytes)   | Description                                          |
|------------------|----------|------------------|------------------------------------------------------|
| format_version   | `[u8]`   | 3                | version of the specification                         |
| id               | `u64`    | 8                | id of the event                                      |
| ref_id_exists    | `u8`     | 1                | 0xFF indicates that a ref id exists and must be read |
| ref_id           | `u64`    | 8                | ref id. only when the indicator is 0xFF              |
| namespace_length | `u16`    | 2                | length of the namespace. 0 means there's none        |
| namespace        | `String` | namespace_length | namespace of the event                               |
| name_length      | `u16`    | 2                | length of the event name                             |
| name             | `String` | name_length      | name of the event                                    |

The header format ensures that it can be read without knowing its length.
That means that a valid header can be deserialized even if the length of the header bytes
is longer. Additional header fields can therefore be appended without having to worry about
backwards compatibility of the format.


## Binary Representation of Special Payloads

### Raw Payload

The raw payload is a `Vec<u8>` and written as is without serialization or deserialization.


### Tandem Payload

The tandem payload contains two inner payloads which can be serialized and deserialized
independently.
Its layout is as follows:

| Name            | Type  | Length (bytes)  | Description                  |
|-----------------|-------|-----------------|------------------------------|
| payload1_length | `u64` | 8               | length of the first payload  |
| payload1        | `T1`  | payload1_length | the first payload            |
| payload2_length | `u64` | 8               | length of the second payload |
| payload2        | `T2`  | payload2_length | the second payload           |


### Serde Payload

The serde payload stores an encoded payload with additional information about the format
the data was serialized as.

| Name      | Type | Length (bytes) | Description                              |
|-----------|------|----------------|------------------------------------------|
| format_id | `u8` | 1              | the format the payload was serialized as |
| payload   | `T`  | ~              | the serialized payload                   |


## Behaviour

### Receiving events

When receiving an event the handler registered for the name of the event is called.
The event will be ignored if no handler is registered.


### Receiving namespaced events

Namespaced events are handled similar to regular event handling. Instead of searching 
for a handler that handles the event with the given name, first the namespace for the
event is retrieved. On the namespace the handler registered for that specific event is called.
If no namespace for the event namespace is registered or no handler is registered for
the event name, the event will be ignored.


### Receiving replies to emitted events

When emitting an event to a peer, the emitter can wait for an answer to that event.
This is achieved by emitting events as a response to a specific event id.
When an event with a reference event id (ref_id) is received, first the registry is
searched for handlers waiting for a response (by trying to receive from a channel).
If a handler can be found, the event is passed to the handler waiting for the response.
Otherwise, the event will be processed as a regular event.
Events passed from an event handler are always passed as replies to the event that
called that handler.