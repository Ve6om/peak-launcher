/** A readable name for a Java install from its java binary's path: the
 *  folder it lives in, e.g. "graalvm-jdk-21.0.12" or "jdk-21.0.13.9-hotspot". */
export function javaFolderName(path: string): string {
  const parts = path.split(/[\/]+/).filter(Boolean);
  let i = parts.lastIndexOf('bin');
  if (i < 1) return path;
  // macOS: <name>/Contents/Home/bin
  if (parts[i - 1] === 'Home' && parts[i - 2] === 'Contents' && i >= 3) i -= 2;
  return parts[i - 1];
}
