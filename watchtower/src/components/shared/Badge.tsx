import { useColors } from "../../hooks";

interface BadgeProps {
  label: string;
  variant?: "success" | "warning" | "error" | "info" | "muted";
}

export function Badge({ label, variant = "info" }: BadgeProps) {
  const colors = useColors();
  const bgColor =
    variant === "success"
      ? colors.success
      : variant === "warning"
        ? colors.warning
        : variant === "error"
          ? colors.error
          : variant === "muted"
            ? colors.textDim
            : colors.accent;

  return (
    <box
      style={{
        paddingLeft: 1,
        paddingRight: 1,
        backgroundColor: bgColor,
      }}
    >
      <text fg={colors.bg}>{label}</text>
    </box>
  );
}
