// Meilisearch search toolbar.
//
// Contributes a search box to the `data-grid.toolbar.actions` slot. Because UI
// extensions never receive the connection host/API key (by design), the search
// runs through `usePluginQuery`, which routes back to this plugin's driver via
// a `@meili-search:{json}` sentinel — the driver turns it into a real
// Meilisearch search (typo tolerance, ranking) and returns the hits, which we
// show in a host modal.

import {
  defineSlot,
  usePluginModal,
  usePluginQuery,
  usePluginToast,
  usePluginTranslation,
} from "@tabularis/plugin-api";

const PLUGIN_ID = "meilisearch";
const SEARCH_PREFIX = "@meili-search:";
const LIMIT = 20;

type QueryResult = { columns: string[]; rows: unknown[][] };

function cell(value: unknown): string {
  if (value === null || value === undefined) return "";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

function ResultsTable({
  result,
  emptyLabel,
}: {
  result: QueryResult;
  emptyLabel: string;
}) {
  if (!result.rows.length) {
    return <div style={{ padding: "12px", opacity: 0.7 }}>{emptyLabel}</div>;
  }
  return (
    <div style={{ overflow: "auto", maxHeight: "60vh" }}>
      <table
        style={{ borderCollapse: "collapse", width: "100%", fontSize: "12px" }}
      >
        <thead>
          <tr>
            {result.columns.map((c) => (
              <th
                key={c}
                style={{
                  textAlign: "left",
                  padding: "4px 8px",
                  borderBottom: "1px solid currentColor",
                  position: "sticky",
                  top: 0,
                  background: "inherit",
                }}
              >
                {c}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {result.rows.map((row, i) => (
            <tr key={i}>
              {row.map((value, j) => (
                <td
                  key={j}
                  style={{
                    padding: "4px 8px",
                    borderBottom: "1px solid rgba(127,127,127,0.25)",
                    whiteSpace: "nowrap",
                    maxWidth: "320px",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                  }}
                  title={cell(value)}
                >
                  {cell(value)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

const Toolbar = defineSlot("data-grid.toolbar.actions", ({ context }) => {
  const t = usePluginTranslation(PLUGIN_ID);
  const { showError } = usePluginToast();
  const { openModal } = usePluginModal();
  const runQuery = usePluginQuery();

  let term = "";

  const runSearch = async () => {
    const index = context.tableName;
    if (!index) {
      showError(t("toolbar.noIndex"));
      return;
    }
    try {
      const sentinel =
        SEARCH_PREFIX + JSON.stringify({ index, q: term, limit: LIMIT });
      const result = (await runQuery(sentinel)) as QueryResult;
      openModal({
        title: t("toolbar.resultsTitle", { index }),
        size: "lg",
        content: (
          <ResultsTable result={result} emptyLabel={t("toolbar.empty")} />
        ),
      });
    } catch (err) {
      showError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div style={{ display: "flex", gap: "4px", alignItems: "center" }}>
      <input
        type="search"
        placeholder={t("toolbar.placeholder")}
        onChange={(e) => {
          term = e.target.value;
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") void runSearch();
        }}
        style={{
          padding: "4px 8px",
          fontSize: "12px",
          border: "1px solid currentColor",
          borderRadius: "4px",
          background: "transparent",
          color: "inherit",
          minWidth: "160px",
        }}
      />
      <button
        type="button"
        onClick={() => void runSearch()}
        style={{
          padding: "4px 10px",
          fontSize: "12px",
          border: "1px solid currentColor",
          borderRadius: "4px",
          background: "transparent",
          color: "inherit",
          cursor: "pointer",
        }}
      >
        {t("toolbar.search")}
      </button>
    </div>
  );
});

export default Toolbar.component;
