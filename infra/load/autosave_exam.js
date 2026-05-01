import http from "k6/http";
import { check, sleep } from "k6";

export const options = {
  scenarios: {
    autosave_exam: {
      executor: "constant-vus",
      vus: Number(__ENV.VUS || 50),
      duration: __ENV.DURATION || "2m",
    },
  },
  thresholds: {
    http_req_duration: ["p(95)<500"],
    http_req_failed: ["rate<0.01"],
  },
};

const baseUrl = __ENV.BASE_URL || "http://localhost:8080";
const tenantId = __ENV.TENANT_ID || "00000000-0000-0000-0000-000000000001";
const userId = __ENV.USER_ID || "00000000-0000-0000-0000-000000000002";
const permissions = __ENV.PERMISSIONS || "assessments:submit";

export default function () {
  const headers = {
    "content-type": "application/json",
    "x-tenant-id": tenantId,
    "x-user-id": userId,
    "x-permissions": permissions,
    "idempotency-key": `attempt-${__VU}-${__ITER}`,
  };

  const body = JSON.stringify({
    questionId: "00000000-0000-0000-0000-000000000003",
    answer: { type: "mcq", selectedOptionIds: ["00000000-0000-0000-0000-000000000004"] },
  });

  const res = http.post(`${baseUrl}/api/v1/attempts/00000000-0000-0000-0000-000000000005/answers`, body, { headers });
  check(res, {
    "autosave accepted": (r) => r.status === 200,
  });
  sleep(Number(__ENV.AUTOSAVE_INTERVAL_SECONDS || 3));
}
