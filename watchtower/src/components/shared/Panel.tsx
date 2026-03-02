import type { ReactNode } from "react";
import { useColors } from "../../hooks";

interface PanelProps {
  title?: string;
  children: ReactNode;
  flexGrow?: number;
  marginTop?: number;
  marginBottom?: number;
}

export function Panel({
  title,
  children,
  flexGrow = 0,
  marginTop = 0,
  marginBottom = 0,
}: PanelProps) {
  const colors = useColors();
  return (
    <box
      title={title ? ` ${title} ` : undefined}
      style={{
        border: true,
        borderColor: colors.border,
        padding: 1,
        flexGrow,
        marginTop,
        marginBottom,
      }}
    >
      {children}
    </box>
  );
}
