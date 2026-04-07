export const metadata = {
  title: 'Poulets BFF',
  description: 'FASO DIGITALISATION - Poulets Platform BFF',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="fr">
      <body>{children}</body>
    </html>
  );
}
