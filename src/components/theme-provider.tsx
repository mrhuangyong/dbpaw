import { createContext, useContext, useEffect, useState } from "react";
import { getSetting, saveSetting } from "@/services/store";
import {
  ThemeId,
  getThemeAppearance,
  normalizeThemeId,
} from "@/theme/themeRegistry";

export type Theme = ThemeId;
export const MIN_FONT_SIZE_PX = 10;
export const MAX_FONT_SIZE_PX = 24;
export const DEFAULT_FONT_SIZE_PX = 14;

export const MIN_EDITOR_FONT_SIZE_PX = 8;
export const MAX_EDITOR_FONT_SIZE_PX = 32;
export const DEFAULT_EDITOR_FONT_SIZE_PX = 14;

export const DEFAULT_FONT_FAMILY = "system-ui, -apple-system, sans-serif";

interface ThemeProviderState {
  theme: ThemeId;
  setTheme: (theme: ThemeId) => void;
  fontSizePx: number;
  setFontSizePx: (size: number) => void;
  editorFontSizePx: number;
  setEditorFontSizePx: (size: number) => void;
  fontFamily: string;
  setFontFamily: (font: string) => void;
}

const initialState: ThemeProviderState = {
  theme: "default",
  setTheme: () => null,
  fontSizePx: DEFAULT_FONT_SIZE_PX,
  setFontSizePx: () => null,
  editorFontSizePx: DEFAULT_EDITOR_FONT_SIZE_PX,
  setEditorFontSizePx: () => null,
  fontFamily: DEFAULT_FONT_FAMILY,
  setFontFamily: () => null,
};

const ThemeProviderContext = createContext<ThemeProviderState>(initialState);

export function ThemeProvider({
  children,
  defaultTheme = "default",
  ...props
}: {
  children: React.ReactNode;
  defaultTheme?: ThemeId;
}) {
  const [theme, setThemeState] = useState<ThemeId>(defaultTheme);
  const [fontSizePx, setFontSizePxState] =
    useState<number>(DEFAULT_FONT_SIZE_PX);
  const [editorFontSizePx, setEditorFontSizePxState] = useState<number>(
    DEFAULT_EDITOR_FONT_SIZE_PX,
  );
  const [fontFamily, setFontFamilyState] =
    useState<string>(DEFAULT_FONT_FAMILY);
  const [isLoaded, setIsLoaded] = useState(false);

  const clampFontSize = (size: number) => {
    if (!Number.isFinite(size)) {
      return DEFAULT_FONT_SIZE_PX;
    }

    const rounded = Math.round(size);
    return Math.min(MAX_FONT_SIZE_PX, Math.max(MIN_FONT_SIZE_PX, rounded));
  };

  const clampEditorFontSize = (size: number) => {
    if (!Number.isFinite(size)) {
      return DEFAULT_EDITOR_FONT_SIZE_PX;
    }

    const rounded = Math.round(size);
    return Math.min(
      MAX_EDITOR_FONT_SIZE_PX,
      Math.max(MIN_EDITOR_FONT_SIZE_PX, rounded),
    );
  };

  const applyTheme = (themeId: ThemeId) => {
    const root = document.documentElement;
    const appearance = getThemeAppearance(themeId);

    root.setAttribute("data-theme", themeId);
    root.classList.remove("light", "dark");
    root.classList.add(appearance);
    root.style.colorScheme = appearance;
  };

  const applyFontSizePx = (size: number) => {
    const root = document.documentElement;
    root.style.setProperty("--font-size", `${size}px`);
  };

  const applyFontFamily = (font: string) => {
    const root = document.documentElement;
    root.style.setProperty("--font-family", font);
  };

  useEffect(() => {
    const loadSettings = async () => {
      const rawTheme = await getSetting<string>("theme", defaultTheme);
      const savedTheme = normalizeThemeId(rawTheme);
      const savedFontSize = await getSetting<number>(
        "fontSizePx",
        DEFAULT_FONT_SIZE_PX,
      );
      const normalizedFontSize = clampFontSize(savedFontSize);
      const savedEditorFontSize = await getSetting<number>(
        "editorFontSizePx",
        DEFAULT_EDITOR_FONT_SIZE_PX,
      );
      const normalizedEditorFontSize = clampEditorFontSize(savedEditorFontSize);
      const savedFontFamily = await getSetting<string>(
        "fontFamily",
        DEFAULT_FONT_FAMILY,
      );

      setThemeState(savedTheme);
      setFontSizePxState(normalizedFontSize);
      setEditorFontSizePxState(normalizedEditorFontSize);
      setFontFamilyState(savedFontFamily);

      applyTheme(savedTheme);
      applyFontSizePx(normalizedFontSize);
      applyFontFamily(savedFontFamily);

      if (savedTheme !== rawTheme) {
        void saveSetting("theme", savedTheme);
      }

      setIsLoaded(true);
    };

    void loadSettings();
  }, [defaultTheme]);

  const setTheme = (themeId: ThemeId) => {
    setThemeState(themeId);
    applyTheme(themeId);
    void saveSetting("theme", themeId);
  };

  const setFontSizePx = (size: number) => {
    const normalizedSize = clampFontSize(size);
    setFontSizePxState(normalizedSize);
    applyFontSizePx(normalizedSize);
    void saveSetting("fontSizePx", normalizedSize);
  };

  const setEditorFontSizePx = (size: number) => {
    const normalizedSize = clampEditorFontSize(size);
    setEditorFontSizePxState(normalizedSize);
    void saveSetting("editorFontSizePx", normalizedSize);
  };

  const setFontFamily = (font: string) => {
    setFontFamilyState(font);
    applyFontFamily(font);
    void saveSetting("fontFamily", font);
  };

  if (!isLoaded) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-background">
        <div className="text-muted-foreground text-sm">Loading...</div>
      </div>
    );
  }

  const value = {
    theme,
    setTheme,
    fontSizePx,
    setFontSizePx,
    editorFontSizePx,
    setEditorFontSizePx,
    fontFamily,
    setFontFamily,
  };

  return (
    <ThemeProviderContext.Provider {...props} value={value}>
      {children}
    </ThemeProviderContext.Provider>
  );
}

export const useTheme = () => {
  const context = useContext(ThemeProviderContext);

  if (context === undefined)
    throw new Error("useTheme must be used within a ThemeProvider");

  return context;
};
