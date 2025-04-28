# vanilla-discord-bridge

A Discord bridge supporting modern vanilla servers (and modded ones, too), optionally providing two-way communication if you use the wrapper mode.

## Setup

Change the `apiServer` and `apiKey` as appropriate - you may use a blank API key if you are sure that the API server is inaccessible from the wider internet. Otherwise, people can send fake messages to your Discord bridge. Ruh-roh!

We set the chat endpoint to "v1/chatx" so that the Minecraft server sends a more consistent request. TODO: Does this work in the earliest version of the game supporting the filter system?

```properties
text-filtering-version=0
text-filtering-config={"apiServer":"http://127.0.0.1:8080/","apiKey":"API_KEY","endpoints":{"chat":"v1/chatx"}}
```