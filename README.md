# vanilla-discord-bridge

> [!CAUTION]
> This project is a curiosity more than anything and may not be secure or stable enough to deploy in prod. However, I have published the source code as it may be useful for snapshot test servers. If you're looking for a Discord bridge for a regular server, though, you probably want to use Fabric/Paper instead of vanilla so you can install a Discord bridge mod/plugin. Here be dragons & etc.

A Discord bridge supporting modern vanilla servers (and modded ones, too), optionally providing two-way communication if you use the wrapper mode.

Lack of rich content support (i.e. attachments, stickers, display names/global names) is intentional so that this software works with as many Minecraft versions & Discord updates as possible.

## Server Setup

Change the `apiServer` and `apiKey` as appropriate - you may use a blank API key if you are sure that the API server is inaccessible from the wider internet. Otherwise, people can send fake messages to your Discord bridge. Ruh-roh!

We set the chat endpoint to "v1/chatx" so that the Minecraft server sends a more consistent request. TODO: Does this work in the earliest version of the game supporting the filter system?

```properties
text-filtering-version=0
text-filtering-config={"apiServer":"http://127.0.0.1:8080/","apiKey":"API_KEY","endpoints":{"chat":"v1/chatx"}}
```

### Wrapper Mode

In order to allow Discord users to communicate with Minecraft servers via Minecraft chat, vanilla-discord-bridge must launch your server and inject tellraw into the process input. You will still be able to input commands via process input, but you will not be able to tab complete them on server implementations that support console tab completions.

You must additionally create a Discord bot and make sure it has the `Message Content` gateway intent enabled.

To launch the server wrapped by vanilla-discord-bridge, invoke vanilla-discord-bridge with the full command line you use to launch your server without the wrapper (i.e. `PERHAPS_SOME_ENV_HERE=1 java -jar server.jar --nogui` would become `PERHAPS_SOME_ENV_HERE=1 API_KEY=SECURE_API_KEY WEBHOOK_ID=00000000000 WEBHOOK_TOKEN=WEBHOOK_TOKEN_HERE DISCORD_TOKEN=DISCORD_TOKEN_HERE DISCORD_CHANNEL_ID=00000000000 vanilla-discord-bridge java -jar server.jar --nogui`). Specifying DISCORD_TOKEN requires that you specify DISCORD_CHANNEL_ID, and vice-versa.


## Environment variables

|Name|Default|Type|Description|
|-|-|-|-|
|API_KEY|-|String|The API key we check to ensure the filter request originated from the server. This is most important when you are running the server in standalone mode on a different machine, although that setup is not recommended as it will noticeably increase message latency if not over LAN.|
|BIND_ADDRESS|127.0.0.1:8080|Socket Address|Address the filter server should listen on. This needs to be synced with server.properties|
|WEBHOOK_ID|-|u64|The numeric ID of the webhook.|
|WEBHOOK_TOKEN|-|String|The token of the webhook.|
|DISCORD_TOKEN|-|String|Token of the Discord bot for wrapper mode.|
|DISCORD_CHANNEL_ID|-|u64|ID of the bridge channel so we can ignore messages from elsewhere.|
|ALLOW_EVERYONE_MENTION|false|bool|Should Minecraft users be able to mention @everyone?|
|ALLOW_USER_MENTION|false|bool|Should Minecraft users be able to mention users?|
|ALLOW_ROLE_MENTION|false|bool|Should Minecraft users be able to mention roles?|
|EMBED_URL|false|bool|Should URLs sent by Minecraft users embed on Discord?|