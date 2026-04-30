// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Carte interactive Leaflet pour les parcelles TERROIR.
// Tiles : OpenStreetMap (CC BY-SA, souverain via mirror Nominatim si dispo
// en dev — fallback tile.openstreetmap.org).
import { MapContainer, TileLayer, GeoJSON, Marker, Popup } from 'react-leaflet';
import type { Feature, FeatureCollection, Geometry } from 'geojson';
import type { Parcel, EudrStatus } from '../api/types';
import { useTranslation } from 'react-i18next';
import { useMemo } from 'react';

interface ParcelMapProps {
  parcels: Parcel[];
  center?: [number, number];
  zoom?: number;
  height?: number;
  onParcelClick?: (parcelId: string) => void;
}

function colorForStatus(status: EudrStatus): string {
  switch (status) {
    case 'validated':
      return '#1b5e20';
    case 'rejected':
      return '#e53935';
    case 'escalated':
      return '#f57c00';
    case 'expired':
      return '#9e9e9e';
    case 'pending':
    default:
      return '#1565c0';
  }
}

function parcelToFeature(p: Parcel): Feature<Geometry, { id: string; status: EudrStatus }> {
  return {
    type: 'Feature',
    geometry: p.geojson as Geometry,
    properties: { id: p.id, status: p.eudr_status },
  };
}

// Burkina Faso geographic centroid (Ouagadougou area).
const DEFAULT_CENTER: [number, number] = [12.3714, -1.5197];
const DEFAULT_ZOOM = 7;

export function ParcelMap({
  parcels,
  center = DEFAULT_CENTER,
  zoom = DEFAULT_ZOOM,
  height = 600,
  onParcelClick,
}: ParcelMapProps) {
  const { t } = useTranslation();

  const featureCollection = useMemo<FeatureCollection>(
    () => ({
      type: 'FeatureCollection',
      features: parcels.map(parcelToFeature),
    }),
    [parcels],
  );

  return (
    <div className="map-container" style={{ height }}>
      <MapContainer
        center={center}
        zoom={zoom}
        style={{ height: '100%', width: '100%' }}
      >
        <TileLayer
          attribution='&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors (CC BY-SA)'
          url="https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png"
        />
        <GeoJSON
          key={parcels.map((p) => `${p.id}:${p.eudr_status}`).join('|')}
          data={featureCollection}
          style={(feature) => {
            const status = (feature?.properties?.status ?? 'pending') as EudrStatus;
            const color = colorForStatus(status);
            return {
              color,
              weight: 2,
              fillColor: color,
              fillOpacity: 0.25,
            };
          }}
          onEachFeature={(feature, layer) => {
            const props = feature.properties as { id: string };
            layer.on('click', () => {
              if (onParcelClick) onParcelClick(props.id);
            });
          }}
        />
        {parcels.map((p) => (
          <Marker key={p.id} position={[p.centroid.lat, p.centroid.lon]}>
            <Popup>
              <div style={{ minWidth: 180 }}>
                <strong>{p.crop_type.toUpperCase()}</strong>
                <div style={{ fontSize: 12 }}>
                  {t('terroir.parcels.popup.surface')}: {p.surface_ha.toFixed(2)} ha
                </div>
                <div style={{ fontSize: 12 }}>
                  {t('terroir.parcels.popup.status')}:{' '}
                  <span style={{ color: colorForStatus(p.eudr_status) }}>
                    {t(`terroir.eudr.status.${p.eudr_status}`)}
                  </span>
                </div>
                <a href={`/parcels/${p.id}`} style={{ fontSize: 12 }}>
                  {t('terroir.parcels.popup.open')}
                </a>
              </div>
            </Popup>
          </Marker>
        ))}
      </MapContainer>
    </div>
  );
}
