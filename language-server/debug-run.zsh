#! /bin/zsh

# Get log level from command line argument or default to "debug"
LOG_LEVEL=${1:-debug}

# Print the log level being used
echo "Using log level: $LOG_LEVEL"

RUST_LOG=$LOG_LEVEL cargo run
