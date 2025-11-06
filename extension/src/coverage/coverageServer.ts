import * as http from "http";
import * as vscode from "vscode";
import * as net from "net";
import { SOLANA_OUTPUT_CHANNEL } from "../output";
import { EventEmitter } from "events";

import { CoverageServerConstants } from "./constants";

const {
  DEFAULT_COVERAGE_SERVER_PORT,
  UPDATE_DECORATIONS,
  SETUP_DYNAMIC_COVERAGE,
  DISPLAY_FINAL_REPORT,
} = CoverageServerConstants;

/**
 * HTTP server that receives coverage notifications from external processes
 * Extends EventEmitter to notify coverage manager of incoming requests
 * Handles JSON payloads and routes requests based on URL endpoints
 */
class CoverageServer extends EventEmitter {
  /** HTTP server instance */
  private server: http.Server;
  /** Port number for the HTTP server */
  private port: number;
  /** Output channel for logging */
  private outputChannel: vscode.OutputChannel;

  /**
   * Override emit to also emit an 'any' event with the event name
   * This allows the coverage manager to listen for all events with a single listener
   * @param event - The event name to emit
   * @param args - Arguments to pass with the event
   * @returns {boolean} True if the event had listeners, false otherwise
   */
  emit(event: string | symbol, ...args: any[]): boolean {
    const result = super.emit(event, ...args);
    if (event !== "any") {
      super.emit("any", event, ...args);
    }
    return result;
  }

  /**
   * Creates a new CoverageServer instance and starts the HTTP server
   * Reads the port configuration from VS Code settings
   */
  constructor() {
    super();
    this.port = DEFAULT_COVERAGE_SERVER_PORT;
    this.outputChannel = SOLANA_OUTPUT_CHANNEL;
    this.server = this.setupServer();
  }

  /**
   * Disposes of the HTTP server and closes all connections
   * @public
   */
  public dispose() {
    this.server.close();
  }

  /**
   * Sets up and configures the HTTP server to handle POST requests
   * Parses JSON request bodies and routes requests to appropriate handlers
   * @private
   * @returns {http.Server} The configured HTTP server instance
   */
  private setupServer(): http.Server {
    this.server = http.createServer((req, res) => {
      if (req.method !== "POST") {
        console.error(`Invalid request method: ${req.method}`);
        res.writeHead(405);
        res.end();
        return;
      }

      let body = "";
      req.on("data", (chunk) => {
        body += chunk.toString();
      });

      req.on("end", () => {
        try {
          const data = body ? JSON.parse(body) : {};
          this.handleNotification(req, data);
        } catch (error) {
          console.error("Error parsing JSON:", error);
          this.handleNotification(req, {});
        }

        res.writeHead(200);
        res.end();
      });
    });

    this.outputChannel.appendLine(
      `Starting coverage server on port ${this.port}`
    );

    this.findAvailablePort(this.port)
      .then((freePort) => {
        this.port = freePort;
        this.server.listen(this.port, "localhost", () => {
          const message = `Coverage server running on port: ${this.port}`;
          console.log(message);
          this.outputChannel.appendLine(message);
          vscode.window.showInformationMessage(message);
        });
      })
      .catch((error) => {
        console.error(`Failed to start coverage server: ${error}`);
        this.outputChannel.appendLine(
          `Failed to start coverage server: ${error}`
        );
      });

    // Keep a generic error handler for runtime errors; ignore EADDRINUSE which is handled above
    this.server.on("error", (error: any) => {
      if ((error as NodeJS.ErrnoException).code === "EADDRINUSE") {
        return;
      }
      console.error(`HTTP server error: ${error}`);
    });

    return this.server;
  }

  /**
   * Finds an available port by iteratively probing upward from the starting port
   * @param {number} startPort - The starting port to probe from
   * @returns {Promise<number>} The available port number
   */
  private async findAvailablePort(startPort: number): Promise<number> {
    let port = startPort;
    while (true) {
      const available = await new Promise<boolean>((resolve) => {
        const tester = net.createServer();
        tester.once("error", (err: any) => {
          if ((err as NodeJS.ErrnoException).code === "EADDRINUSE") {
            this.outputChannel.appendLine(
              `Coverage server port ${port} in use; trying ${port + 1}...`
            );
            tester.close(() => resolve(false));
          } else {
            this.outputChannel.appendLine(
              `HTTP port check error on ${port}: ${err}`
            );
            tester.close(() => resolve(false));
          }
        });
        tester.once("listening", () => {
          tester.close(() => resolve(true));
        });
        tester.listen(port, "localhost");
      });

      if (available) {
        return port;
      }
      port += 1;
    }
  }

  /**
   * Handles incoming HTTP notification requests and emits appropriate events
   * Routes requests based on URL path and emits events for the coverage manager
   * @private
   * @param {http.IncomingMessage} req - The incoming HTTP request
   * @param {any} data - Parsed JSON data from the request body
   */
  private handleNotification(req: http.IncomingMessage, data: any) {
    switch (req.url) {
      case SETUP_DYNAMIC_COVERAGE:
        this.emit(SETUP_DYNAMIC_COVERAGE);
        break;
      case UPDATE_DECORATIONS:
        this.emit(UPDATE_DECORATIONS);
        break;
      case DISPLAY_FINAL_REPORT:
        this.emit(DISPLAY_FINAL_REPORT, data);
        break;
      default:
        console.error(`Invalid request URL: ${req.url}`);
    }
  }
}

export { CoverageServer };
