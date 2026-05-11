# M5b — Manual Validation Notes

## §3 Wire shapes

- `core.session.confirm_request` `details.taint` is serialised as
  `Vec<TaintEntry>` — a JSON array of `{source, detail?}` objects
  (`detail` omitted when `None`). When the inbound
  `core.session.tool_request` envelope has no taint, the array
  renders as `[]` (an empty array), never `null`. §CD1 / §CD3.
