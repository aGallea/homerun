interface StatsCardProps {
  label: string;
  value: string | number;
  color?: string;
}

export function StatsCard({ label, value, color }: StatsCardProps) {
  return (
    <div className="stats-card card">
      <div className="stats-card-label">{label}</div>
      <div className="stats-card-value" style={color ? { color } : undefined}>
        {value}
      </div>
    </div>
  );
}
