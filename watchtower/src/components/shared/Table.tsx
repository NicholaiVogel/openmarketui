import type { ReactNode } from "react";
import { useColors } from "../../hooks";

interface Column<T> {
  key: string;
  header: string;
  width?: number;
  render: (item: T, index: number) => ReactNode;
}

interface TableProps<T> {
  columns: Column<T>[];
  data: T[];
  selectedIndex?: number;
  emptyMessage?: string;
}

export function Table<T>({
  columns,
  data,
  selectedIndex,
  emptyMessage = "no data",
}: TableProps<T>) {
  const colors = useColors();
  if (data.length === 0) {
    return <text fg={colors.textDim}>{emptyMessage}</text>;
  }

  return (
    <box style={{ flexDirection: "column", gap: 0 }}>
      {/* header */}
      <box style={{ flexDirection: "row" }}>
        {columns.map((col) => (
          <text
            key={col.key}
            style={{ width: col.width }}
            fg={colors.textDim}
          >
            {col.header}
          </text>
        ))}
      </box>

      {/* rows */}
      {data.map((item, idx) => (
        <box
          key={idx}
          style={{
            flexDirection: "row",
            backgroundColor:
              selectedIndex === idx ? colors.bgAlt : undefined,
          }}
        >
          {columns.map((col) => col.render(item, idx))}
        </box>
      ))}
    </box>
  );
}
