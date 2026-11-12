// Small helpers shared by every page.

// Talk to the OnePass server. Returns the JSON reply, or throws the server's error message.
async function api(path, body) {
  const res = await fetch(path, body === undefined ? {} : {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  const data = await res.json();
  if (!res.ok) throw new Error(data.error || "Something went wrong");
  return data;
}

// Replace everything inside a page element. Text is always inserted as text,
// never as HTML. Lists are unpacked, and empty values (false, null) are skipped.
function fill(node, ...children) {
  node.replaceChildren();
  for (const child of children.flat(Infinity)) {
    if (child === null || child === undefined || child === false) continue;
    node.append(child.nodeType ? child : document.createTextNode(String(child)));
  }
  return node;
}

// Build a new page element with the given attributes and contents.
function el(tag, props = {}, ...children) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(props)) {
    if (key === "class") node.className = value;
    else if (key.startsWith("on")) node.addEventListener(key.slice(2), value);
    else node.setAttribute(key, value);
  }
  return fill(node, ...children);
}

const LABELS = {
  full_name: "Full name",
  address: "Address",
  date_of_birth: "Date of birth",
  is_student: "Student status",
  over_18: "Over 18",
  student_id: "Student ID",
  program: "Program of study",
  enrolment_year: "Enrolment year",
  customer_since: "Bank customer since",
  is_tenant: "Current tenancy",
  tenancy_start: "Tenancy start date",
};
const label = (field) => LABELS[field] || field;
const show = (value) => (value === true ? "Yes" : value === false ? "No" : String(value));

// The value after ?name= in the page address, or a fallback.
const queryParam = (name, fallback) => new URLSearchParams(location.search).get(name) || fallback;
