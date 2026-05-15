import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";

const rootDir = dirname(fileURLToPath(import.meta.url));
const desktopDir = join(rootDir, "..");
const testDistDir = join(desktopDir, ".test-dist");
const tscBin = join(desktopDir, "node_modules", "typescript", "bin", "tsc");

try {
  rmSync(testDistDir, { recursive: true, force: true });

  execFileSync(process.execPath, [tscBin, "-p", "tsconfig.unit.json"], {
    cwd: desktopDir,
    stdio: "inherit",
  });

  mkdirSync(testDistDir, { recursive: true });
  writeFileSync(
    join(testDistDir, "package.json"),
    JSON.stringify({ type: "commonjs" }, null, 2),
  );

  execFileSync(
    process.execPath,
    [
      "--test",
      "tests/unit/validationRunner.test.cjs",
      "tests/unit/validationState.test.cjs",
    ],
    {
      cwd: desktopDir,
      stdio: "inherit",
    },
  );
} finally {
  rmSync(testDistDir, { recursive: true, force: true });
}
