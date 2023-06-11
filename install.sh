#!/bin/bash

# Colours
RED="31"
GREEN="32"
BOLDGREEN="\e[1;${GREEN}m"
BOLDRED="\e[1;${RED}m"
ENDCOLOR="\e[0m"

# Installing rust 
rust=$(which cargo)
if [ -z "$rust" ]; then
    echo -e "[$BOLDRED!$ENDCOLOR] Rust is not installed"
    sleep 1
    echo -e "[$BOLDGREEN+$ENDCOLOR] Installing rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
else
    echo -e "[$BOLDGREEN+$ENDCOLOR] Rust is installed"
fi

# install the binary
echo -e "[$BOLDGREEN+$ENDCOLOR] Setting up everything as $(whoami) user..."
sleep 1
# Build the binary in the target/releases directory
echo -e "[$BOLDGREEN+$ENDCOLOR] Compiling the binary..."
cargo build -r
# Copy the binary to /bin and chmod it with the appropriate permissions
echo -e "[$BOLDGREEN+$ENDCOLOR] Copying the binary to /bin"
sudo cp target/release/hrekt /bin/hrekt ; sudo chmod +x /bin/hrekt
sleep 1
echo -e "[$BOLDGREEN+$ENDCOLOR] Copying the binary to /usr/bin"
sudo cp target/release/hrekt /usr/bin/hrekt ; sudo chmod +x /usr/bin/hrekt
sleep 1
echo -e "[$BOLDGREEN+$ENDCOLOR] Copying the binary to ~/.cargo/bin/"
sudo cp target/release/hrekt .cargo/bin/hrekt ; sudo chmod +x .cargo/bin/hrekt
sleep 1
# Print end message
sleep 1
echo "hrekt has been successfully built."
echo "Happy hacking..."

