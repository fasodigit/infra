// SPDX-License-Identifier: AGPL-3.0-or-later

interface LoadingSkeletonProps {
  width?: string | number;
  height?: string | number;
  count?: number;
  style?: React.CSSProperties;
}

export function LoadingSkeleton({
  width = '100%',
  height = 16,
  count = 1,
  style,
}: LoadingSkeletonProps) {
  const items = Array.from({ length: count }, (_, i) => i);
  return (
    <>
      {items.map((i) => (
        <div
          key={i}
          className="skeleton"
          style={{
            width,
            height,
            marginBottom: 8,
            ...style,
          }}
          aria-hidden="true"
        />
      ))}
    </>
  );
}

export function TableSkeleton({ rows = 5, cols = 4 }: { rows?: number; cols?: number }) {
  return (
    <table>
      <tbody>
        {Array.from({ length: rows }).map((_, r) => (
          <tr key={r}>
            {Array.from({ length: cols }).map((_, c) => (
              <td key={c}>
                <div className="skeleton" style={{ height: 14 }} />
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
