![EVE](./assets/logo.png)

EVE lets you control a Minecraft server through Discord. It routes everything from the server `stdout/stderr` to a Discord channel and receives `stdin` commands from Discord and routes them back to the server instance.

![Example](./assets/example.png)
## Configuration
To run EVE, you need to set up a few environment variables.

**Required**
- `DISCORD_TOKEN`: The token of your Discord bot
- `CONSOLE_CHANNEL_ID`: The the ID of the Discord channel which should be used as the console. EVE will pipe every stdout/stderr line from the server into this channel
- `SERVER_FOLDER`: Folder path of the server. E.g. `/srv/server`
- `SERVER_JAR`: Name of the server executable. E.g. `server.jar`
- `SERVER_MEMORY`: Memory in megabytes to assign to the minecraft server. E.g `6144`

**Optional**
- `MAX_PLAYERS`: Max players of your minecraft server. This is only used for the bot presence and if not provided, it won't show the player count there.
- `JVM_FLAGS`: Additional jvm flags to pass to the server instance
- `AUTO_ACCEPT_EULA`: If the EULA should be accepted automatically
- `SERVER_RUN_COMMAND`: If the server needs to be started with a completely different command, you can specify it here. The command will be executed in the server folder. `SERVER_JAR`, `SERVER_MEMORY` and `JVM_FLAGS` will be ignored.
- `RUST_LOG`: Rust log level (Does not affect the server output). Set it to `info` to recieve all information or to `warn` if you just want to receive warnings/errors.

**Backup**
- `BACKUP_FOLDER`(_Optional_): Backup folder path to save server backups into. **Required** when using the `/backup` command.
- `BACKUP_NAME`(_Optional_): Name of the backup file. Is a format string which is used as the input for [`Astrolabe::DateTime::format`](https://docs.rs/astrolabe/latest/astrolabe/struct.DateTime.html#method.format). Default: `'backup'_yyyy_MM_dd_HH_mm'.tar.gz'`
- `BACKUP_COMMAND`(_Optional_): Command to execute when creating a backup. You can use `{BACKUP_FOLDER}`, `{SERVER_FOLDER}`, `{BACKUP_NAME}` which will be replaced with the environment variables. Default: `tar -czf {BACKUP_FOLDER}/{BACKUP_NAME} {SERVER_FOLDER}`

## Running
There are multiple ways to run EVE:
### Docker
The most convenient way is to run it in a Docker container. EVE gets automatically builded and deployed on [Github Packages](https://github.com/GiyoMoon/EVE/pkgs/container/eve) and can be pulled from there.

**Note**: The server is run as a non-root user. You may need to change the permissions of your server folder.

Example run command:
```bash
docker run -d -p 25565:25565 -e DISCORD_TOKEN=YOUR_BOT_TOKEN -e CONSOLE_CHANNEL_ID=YOUR_CHANNEL_ID-e SERVER_PATH=/eve/server -e SERVER_JAR=server.jar -e SERVER_MEMORY=6144 -v /srv/server:/eve/server --name EVE ghcr.io/giyomoon/eve:java17
```
Additional ports can be mapped if you are running a dynmap for example.

**Java version**

EVE gets build for three different Java versions. Java 17, 11 and 8. Depending on the version/type of your Minecraft server, you need to choose the correct version for you.

- `Java 17`: `java17` (`ghcr.io/giyomoon/eve:java17`)
- `Java 11`: `java11` (`ghcr.io/giyomoon/eve:java11`)
- `Java 8`: `java8` (`ghcr.io/giyomoon/eve:java8`)

### Service
It's also possible to directly use the executable and create a system service. There are pre built binaries under the [Releases](https://github.com/GiyoMoon/EVE/releases), but feel free to build EVE for yourself :)
```bash
sudo touch /etc/systemd/system/eve.service
```
Insert this content:
```
[Unit]
Description=EVE
Wants=network-online.target
After=network-online.target

[Service]

# Use a non-root user
User=eve
Group=eve

ExecReload=/bin/kill -HUP $MAINPID
# Path to the executable
ExecStart=/srv/eve
KillMode=process
KillSignal=SIGINT
LimitNOFILE=65536
LimitNPROC=infinity
Restart=on-failure
RestartSec=2

Environment="DISCORD_TOKEN=YOUR_BOT_TOKEN"
Environment="CONSOLE_CHANNEL_ID=YOUR_CHANNEL_ID"
Environment="SERVER_PATH=/srv/server"
Environment="SERVER_JAR=server.jar"
Environment="SERVER_MEMORY=6144"

[Install]
WantedBy=multi-user.target
```
**Note**: In this example I configured the service to run as the user `eve`. It's best practice to never run a minecraft server as root. Make sure your user has permission to execute the executable and access the server folder.

Now, start the service:
```bash
sudo systemctl enable eve.service
sudo systemctl start eve.service
```
