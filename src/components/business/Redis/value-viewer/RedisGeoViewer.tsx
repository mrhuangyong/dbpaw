import { useState, useMemo, useCallback } from "react";
import { MapPin, Plus, Ruler, Search, Trash2, ArrowUpDown } from "lucide-react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type {
  RedisGeoPosition,
  RedisGeoSearchResult,
  RedisKeyExtra,
} from "@/services/api";

interface ZSetMember {
  member: string;
  score: number;
}

interface Props {
  value: ZSetMember[];
  onChange: (v: ZSetMember[]) => void;
  extra?: RedisKeyExtra | null;
  connectionId: number;
  database?: string;
  redisKey: string;
  onRefresh: () => void;
}

const DISTANCE_UNITS = ["m", "km", "ft", "mi"] as const;

export function RedisGeoViewer({
  value,
  onChange,
  extra,
  connectionId,
  database,
  redisKey,
  onRefresh,
}: Props) {
  const [sortAsc, setSortAsc] = useState(true);
  const [showAddRow, setShowAddRow] = useState(false);
  const [newMember, setNewMember] = useState("");
  const [newLon, setNewLon] = useState("");
  const [newLat, setNewLat] = useState("");
  const [addError, setAddError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);

  // Dist tool
  const [showDistTool, setShowDistTool] = useState(false);
  const [distMember1, setDistMember1] = useState("");
  const [distMember2, setDistMember2] = useState("");
  const [distUnit, setDistUnit] = useState("km");
  const [distResult, setDistResult] = useState<number | null>(null);
  const [distLoading, setDistLoading] = useState(false);

  // POS lookup
  const [posLookup, setPosLookup] = useState<Map<string, RedisGeoPosition>>(
    new Map(),
  );
  const [loadingPos, setLoadingPos] = useState<Set<string>>(new Set());

  // Search
  const [showSearch, setShowSearch] = useState(false);
  const [searchCenter, setSearchCenter] = useState("");
  const [searchRadius, setSearchRadius] = useState("");
  const [searchUnit, setSearchUnit] = useState("km");
  const [searchResults, setSearchResults] = useState<
    RedisGeoSearchResult[] | null
  >(null);
  const [searchLoading, setSearchLoading] = useState(false);

  const geoCount = extra?.geoCount ?? value.length;

  const sorted = useMemo(
    () =>
      [...value].sort((a, b) =>
        sortAsc ? a.score - b.score : b.score - a.score,
      ),
    [value, sortAsc],
  );

  const members = useMemo(() => value.map((m) => m.member), [value]);

  const lookupPos = useCallback(
    async (member: string) => {
      if (posLookup.has(member) || loadingPos.has(member)) return;
      setLoadingPos((prev) => new Set(prev).add(member));
      try {
        const { api } = await import("@/services/api");
        const positions = await api.redis.geoPos(
          connectionId,
          database,
          redisKey,
          [member],
        );
        if (positions[0]) {
          setPosLookup((prev) => new Map(prev).set(member, positions[0]!));
        } else {
          toast.warning("No coordinates found for this member");
        }
      } catch (e) {
        toast.error("Failed to lookup coordinates", {
          description: e instanceof Error ? e.message : String(e),
        });
      } finally {
        setLoadingPos((prev) => {
          const next = new Set(prev);
          next.delete(member);
          return next;
        });
      }
    },
    [connectionId, database, redisKey, posLookup, loadingPos],
  );

  const handleAdd = useCallback(async () => {
    const m = newMember.trim();
    const lon = parseFloat(newLon);
    const lat = parseFloat(newLat);
    if (!m) {
      setAddError("Member name is required");
      return;
    }
    if (isNaN(lon) || lon < -180 || lon > 180) {
      setAddError("Longitude must be between -180 and 180");
      return;
    }
    if (isNaN(lat) || lat < -85.05112878 || lat > 85.05112878) {
      setAddError("Latitude must be between -85.05 and 85.05");
      return;
    }
    setAddError(null);
    setAdding(true);
    try {
      const { api } = await import("@/services/api");
      await api.redis.geoAdd(connectionId, database, redisKey, [
        { member: m, longitude: lon, latitude: lat },
      ]);
      toast.success(`Location "${m}" added`);
      setNewMember("");
      setNewLon("");
      setNewLat("");
      setShowAddRow(false);
      onRefresh();
    } catch (e) {
      toast.error("Failed to add location", {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setAdding(false);
    }
  }, [newMember, newLon, newLat, connectionId, database, redisKey, onRefresh]);

  const handleDist = useCallback(async () => {
    if (!distMember1 || !distMember2) return;
    setDistLoading(true);
    try {
      const { api } = await import("@/services/api");
      const result = await api.redis.geoDist(
        connectionId,
        database,
        redisKey,
        distMember1,
        distMember2,
        distUnit,
      );
      setDistResult(result);
      toast.success("Distance calculated");
    } catch (e) {
      setDistResult(null);
      toast.error("Failed to calculate distance", {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setDistLoading(false);
    }
  }, [distMember1, distMember2, distUnit, connectionId, database, redisKey]);

  const handleSearch = useCallback(async () => {
    const center = searchCenter.trim();
    const radius = parseFloat(searchRadius);
    if (!center || isNaN(radius) || radius <= 0) return;
    setSearchLoading(true);
    try {
      const { api } = await import("@/services/api");
      const results = await api.redis.geoSearch(
        connectionId,
        database,
        redisKey,
        {
          member: center,
          radius,
          unit: searchUnit,
          withCoord: true,
          withDist: true,
        },
      );
      setSearchResults(results);
      toast.success(`Found ${results.length} location(s) nearby`);
    } catch (e) {
      setSearchResults(null);
      toast.error("Failed to search nearby locations", {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setSearchLoading(false);
    }
  }, [
    searchCenter,
    searchRadius,
    searchUnit,
    connectionId,
    database,
    redisKey,
  ]);

  const deleteMember = useCallback(
    (member: string) => {
      onChange(value.filter((m) => m.member !== member));
    },
    [value, onChange],
  );

  return (
    <div className="space-y-3">
      {/* Header */}
      <div className="flex items-center gap-2 flex-wrap">
        <div className="flex items-center gap-1.5">
          <MapPin className="w-4 h-4 text-teal-600" />
          <span className="text-sm font-medium">Geo</span>
        </div>
        <Badge variant="outline" className="text-xs font-mono">
          {geoCount.toLocaleString()} locations
        </Badge>
        <div className="ml-auto flex gap-1.5">
          <Button
            variant="outline"
            size="sm"
            className="h-7 text-xs"
            onClick={() => setShowDistTool(!showDistTool)}
          >
            <Ruler className="w-3 h-3 mr-1" />
            Distance
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="h-7 text-xs"
            onClick={() => setShowSearch(!showSearch)}
          >
            <Search className="w-3 h-3 mr-1" />
            Nearby
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="h-7"
            onClick={() => setSortAsc((a) => !a)}
          >
            <ArrowUpDown className="w-3 h-3 mr-1" />
            Score {sortAsc ? "↑" : "↓"}
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="h-7"
            onClick={() => setShowAddRow(true)}
          >
            <Plus className="w-3 h-3 mr-1" />
            Add
          </Button>
        </div>
      </div>

      {/* Distance tool */}
      {showDistTool && (
        <div className="rounded-md border bg-muted/20 p-3 space-y-2">
          <div className="text-xs font-medium text-muted-foreground">
            GEODIST — Calculate distance between two members
          </div>
          <div className="flex gap-2 items-center flex-wrap">
            <select
              className="h-7 text-xs border rounded px-2 bg-background"
              value={distMember1}
              onChange={(e) => setDistMember1(e.target.value)}
            >
              <option value="">Member 1</option>
              {members.map((m) => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
            <span className="text-xs text-muted-foreground">↔</span>
            <select
              className="h-7 text-xs border rounded px-2 bg-background"
              value={distMember2}
              onChange={(e) => setDistMember2(e.target.value)}
            >
              <option value="">Member 2</option>
              {members.map((m) => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
            <Select value={distUnit} onValueChange={setDistUnit}>
              <SelectTrigger className="h-7 w-16 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {DISTANCE_UNITS.map((u) => (
                  <SelectItem key={u} value={u}>
                    {u}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Button
              size="sm"
              className="h-7 text-xs"
              onClick={handleDist}
              disabled={distLoading || !distMember1 || !distMember2}
            >
              {distLoading ? "..." : "Calculate"}
            </Button>
          </div>
          {distResult !== null && (
            <div className="text-xs text-teal-700 dark:text-teal-400 font-mono">
              {distResult.toLocaleString(undefined, {
                maximumFractionDigits: 4,
              })}{" "}
              {distUnit}
            </div>
          )}
        </div>
      )}

      {/* Nearby search */}
      {showSearch && (
        <div className="rounded-md border bg-muted/20 p-3 space-y-2">
          <div className="text-xs font-medium text-muted-foreground">
            GEOSEARCH — Find locations near a member
          </div>
          <div className="flex gap-2 items-center flex-wrap">
            <select
              className="h-7 text-xs border rounded px-2 bg-background"
              value={searchCenter}
              onChange={(e) => setSearchCenter(e.target.value)}
            >
              <option value="">Center member</option>
              {members.map((m) => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
            <Input
              className="h-7 font-mono text-xs w-20"
              placeholder="Radius"
              value={searchRadius}
              onChange={(e) => setSearchRadius(e.target.value)}
              inputMode="numeric"
            />
            <Select value={searchUnit} onValueChange={setSearchUnit}>
              <SelectTrigger className="h-7 w-16 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {DISTANCE_UNITS.map((u) => (
                  <SelectItem key={u} value={u}>
                    {u}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Button
              size="sm"
              className="h-7 text-xs"
              onClick={handleSearch}
              disabled={searchLoading || !searchCenter || !searchRadius}
            >
              {searchLoading ? "..." : "Search"}
            </Button>
          </div>
          {searchResults !== null && (
            <div className="space-y-1">
              <div className="text-xs text-muted-foreground">
                {searchResults.length} result(s) found
              </div>
              {searchResults.map((r) => (
                <div
                  key={r.member}
                  className="flex items-center gap-2 text-xs font-mono"
                >
                  <span className="text-foreground">{r.member}</span>
                  {r.distance !== undefined && (
                    <span className="text-muted-foreground">
                      {r.distance.toLocaleString(undefined, {
                        maximumFractionDigits: 2,
                      })}{" "}
                      {searchUnit}
                    </span>
                  )}
                  {r.position && (
                    <span className="text-muted-foreground">
                      ({r.position.longitude.toFixed(6)},{" "}
                      {r.position.latitude.toFixed(6)})
                    </span>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Add row */}
      {showAddRow && (
        <div className="rounded-md border bg-muted/20 p-3 space-y-2">
          <div className="text-xs font-medium text-muted-foreground">
            GEOADD — Add a new location
          </div>
          <div className="flex gap-2 items-center flex-wrap">
            <Input
              className="h-7 font-mono text-xs w-32"
              placeholder="Member name"
              value={newMember}
              onChange={(e) => setNewMember(e.target.value)}
            />
            <Input
              className="h-7 font-mono text-xs w-28"
              placeholder="Longitude"
              value={newLon}
              onChange={(e) => setNewLon(e.target.value)}
              inputMode="decimal"
            />
            <Input
              className="h-7 font-mono text-xs w-28"
              placeholder="Latitude"
              value={newLat}
              onChange={(e) => setNewLat(e.target.value)}
              inputMode="decimal"
            />
            <Button
              size="sm"
              className="h-7 text-xs"
              onClick={handleAdd}
              disabled={adding}
            >
              {adding ? "Adding..." : "Add"}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className="h-7 text-xs"
              onClick={() => {
                setShowAddRow(false);
                setAddError(null);
              }}
            >
              Cancel
            </Button>
          </div>
          {addError && <div className="text-xs text-red-600">{addError}</div>}
        </div>
      )}

      {/* Data table */}
      <div className="rounded-md border overflow-hidden">
        <Table>
          <TableHeader>
            <TableRow className="h-8">
              <TableHead className="w-[40px] text-xs py-1">#</TableHead>
              <TableHead className="text-xs py-1">Member</TableHead>
              <TableHead className="text-xs py-1 text-right">Geohash</TableHead>
              <TableHead className="text-xs py-1 text-right">
                Longitude
              </TableHead>
              <TableHead className="text-xs py-1 text-right">
                Latitude
              </TableHead>
              <TableHead className="w-[40px] py-1" />
            </TableRow>
          </TableHeader>
          <TableBody>
            {sorted.length === 0 ? (
              <TableRow>
                <TableCell
                  colSpan={6}
                  className="text-center text-xs text-muted-foreground py-4"
                >
                  No locations
                </TableCell>
              </TableRow>
            ) : (
              sorted.map((m, i) => {
                const pos = posLookup.get(m.member);
                const isLoadingPos = loadingPos.has(m.member);
                return (
                  <TableRow key={m.member} className="h-8 group">
                    <TableCell className="text-xs text-muted-foreground font-mono py-1">
                      {i + 1}
                    </TableCell>
                    <TableCell className="text-xs font-medium py-1">
                      <span className="font-mono">{m.member}</span>
                    </TableCell>
                    <TableCell className="text-xs font-mono text-muted-foreground text-right py-1 tabular-nums">
                      {m.score.toFixed(0)}
                    </TableCell>
                    <TableCell className="text-xs font-mono text-right py-1 tabular-nums">
                      {pos ? (
                        pos.longitude.toFixed(6)
                      ) : (
                        <button
                          className="text-muted-foreground hover:text-foreground underline-offset-2 hover:underline"
                          onClick={() => lookupPos(m.member)}
                          disabled={isLoadingPos}
                        >
                          {isLoadingPos ? "..." : "lookup"}
                        </button>
                      )}
                    </TableCell>
                    <TableCell className="text-xs font-mono text-right py-1 tabular-nums">
                      {pos ? (
                        pos.latitude.toFixed(6)
                      ) : (
                        <button
                          className="text-muted-foreground hover:text-foreground underline-offset-2 hover:underline"
                          onClick={() => lookupPos(m.member)}
                          disabled={isLoadingPos}
                        >
                          {isLoadingPos ? "..." : "lookup"}
                        </button>
                      )}
                    </TableCell>
                    <TableCell className="py-1">
                      <Button
                        variant="ghost"
                        size="sm"
                        className="h-6 w-6 p-0 opacity-0 group-hover:opacity-100"
                        onClick={() => deleteMember(m.member)}
                      >
                        <Trash2 className="w-3 h-3" />
                      </Button>
                    </TableCell>
                  </TableRow>
                );
              })
            )}
          </TableBody>
        </Table>
      </div>

      {/* Footer info */}
      <div className="text-[10px] text-muted-foreground pt-1 border-t">
        Scores are geohash values. Click "lookup" to fetch real coordinates via
        GEOPOS. Use "Distance" and "Nearby" tools for spatial queries.
      </div>
    </div>
  );
}
