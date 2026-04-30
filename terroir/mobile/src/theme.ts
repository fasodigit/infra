// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Thème TERROIR — palette inspirée de la savane / cultures BF.
 * P1 : à raffiner avec design system FASO (Figma).
 */
export const theme = {
  colors: {
    primary: '#1b5e20', // Vert savane (référence ulpaln couleur splash)
    onPrimary: '#ffffff',
    secondary: '#f9a825', // Or sahel
    background: '#fafafa',
    onBackground: '#1c1b1f',
    surface: '#ffffff',
    error: '#b00020',
    success: '#2e7d32',
    warning: '#f57c00',
    border: '#e0e0e0',
  },
  spacing: {
    xs: 4,
    sm: 8,
    md: 16,
    lg: 24,
    xl: 32,
  },
  radius: {
    sm: 4,
    md: 8,
    lg: 16,
  },
  fontSize: {
    sm: 12,
    md: 14,
    lg: 16,
    xl: 20,
    xxl: 24,
  },
} as const;

export type Theme = typeof theme;
