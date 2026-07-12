import { writeFileSync } from "node:fs";

writeFileSync(
  process.env.AERIAL_TEST_OUTPUT,
  `${process.env.AERIAL_MESSAGE_ID}|${process.env.AERIAL_MESSAGE_BODY}`
);
