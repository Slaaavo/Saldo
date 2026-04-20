import http from "node:http";
import fs from "node:fs";
import path from "node:path";

const OLLAMA_PORT = 11434;
const PROXY_PORT = 8080;
const LOG_DIR = path.join(import.meta.dirname, "ollama-logs");

fs.mkdirSync(LOG_DIR, { recursive: true });

let requestCount = 0;

http
  .createServer((req, res) => {
    const chunks = [];
    req.on("data", (c) => chunks.push(c));
    req.on("end", () => {
      const body = Buffer.concat(chunks).toString();
      const idx = ++requestCount;
      const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
      const logFile = path.join(LOG_DIR, `${idx}_${timestamp}.json`);

      try {
        const parsed = JSON.parse(body);
        fs.writeFileSync(logFile, JSON.stringify(parsed, null, 2));
        const tokenEstimate = Math.round(body.length / 4);
        console.log(
          `[${idx}] ${req.method} ${req.url} | ~${tokenEstimate} tokens (${body.length} chars) → ${logFile}`
        );
      } catch {
        fs.writeFileSync(logFile, body);
        console.log(
          `[${idx}] ${req.method} ${req.url} | ${body.length} chars → ${logFile}`
        );
      }

      const proxy = http.request(
        {
          hostname: "localhost",
          port: OLLAMA_PORT,
          path: req.url,
          method: req.method,
          headers: req.headers,
        },
        (pRes) => {
          res.writeHead(pRes.statusCode, pRes.headers);
          pRes.pipe(res);
        }
      );
      proxy.on("error", (err) => {
        console.error(`[${idx}] Proxy error:`, err.message);
        res.writeHead(502);
        res.end("Bad Gateway");
      });
      proxy.end(body);
    });
  })
  .listen(PROXY_PORT, () => {
    console.log(`Ollama proxy listening on http://localhost:${PROXY_PORT}`);
    console.log(`Forwarding to http://localhost:${OLLAMA_PORT}`);
    console.log(`Logs saved to ${LOG_DIR}/`);
  });
