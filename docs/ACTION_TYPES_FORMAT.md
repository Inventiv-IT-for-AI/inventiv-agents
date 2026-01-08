# Action Types Format - Documentation

## Standardized Format

### Backend (Database)
Format: **`UPPER_CASE_WITH_UNDERSCORE`**
- Example: `PROVIDER_CREATE`, `EXECUTE_TERMINATE`, `INSTANCE_CREATED`

### Frontend (User Display)
Format: **`Title Case With Spaces`**
- Automatic transformation: `action_type.replace(/_/g, ' ').replace(/\b\w/g, l => l.toUpperCase())`
- Examples:
  - `PROVIDER_CREATE` → "Provider Create"
  - `EXECUTE_TERMINATE` → "Execute Terminate"
  - `INSTANCE_CREATED` → "Instance Created"
  - `ARCHIVE_INSTANCE` → "Archive Instance"

## Complete Lifecycle Actions

### 🔵 Instance Creation
| Backend Action | Frontend Label | Icon | Component |
|----------------|----------------|------|-----------|
| `REQUEST_CREATE` | Request Create | ⚡ Zap | API |
| `EXECUTE_CREATE` | Execute Create | 🖥️ Server | Orchestrator |
| `PROVIDER_CREATE` | Provider Create | ☁️ Cloud | Orchestrator |
| `INSTANCE_CREATED` | Instance Created | 🗄️ Database | Orchestrator |

### 🔴 Instance Termination
| Backend Action | Frontend Label | Icon | Component |
|----------------|----------------|------|-----------|
| `REQUEST_TERMINATE` | Request Terminate | ⚡ Zap | API |
| `EXECUTE_TERMINATE` | Execute Terminate | 🖥️ Server | Orchestrator |
| `PROVIDER_TERMINATE` | Provider Terminate | ☁️ Cloud | Orchestrator |
| `INSTANCE_TERMINATED` | Instance Terminated | 🗄️ Database | Orchestrator |

### 📦 Archiving
| Backend Action | Frontend Label | Icon | Component |
|----------------|----------------|------|-----------|
| `ARCHIVE_INSTANCE` | Archive Instance | 📦 Archive | API |

### 🔍 Reconciliation & Monitoring
| Backend Action | Frontend Label | Icon | Component |
|----------------|----------------|------|-----------|
| `PROVIDER_DELETED_DETECTED` | Provider Deleted Detected | ⚠️ AlertTriangle | Orchestrator |

## Legacy Actions (to be progressively removed)
| Backend Action | Replaced by | Notes |
|----------------|-------------|-------|
| `SCALEWAY_CREATE` | `PROVIDER_CREATE` | Provider-specific name, not generic |
| `SCALEWAY_DELETE` | `PROVIDER_TERMINATE` | Provider-specific name, not generic |
| `TERMINATE_INSTANCE` | `EXECUTE_TERMINATE` | Incorrect nomenclature |

## Colors & Styles

### Monitoring Page (Badges)
- **Request**: Blue (`bg-blue-500/600`)
- **Execute**: Purple (`bg-purple-500/600`)
- **Provider**: Orange (`bg-orange-500/600`)
- **Instance**: Green (creation) / Red (termination)
- **Archive**: Gray
- **Anomaly Detection**: Yellow

### Timeline Modal (Cards)
- Left border colored according to action
- Matching light background
- Round icons with same color

## Uniformity Rules

✅ **To do**
- Always use `UPPER_CASE_WITH_UNDERSCORE` in backend
- Always convert to `Title Case With Spaces` in frontend
- Use generic names (no provider names)
- Maintain creation/termination symmetry

❌ **To avoid**
- Mixing formats (e.g., `createInstance` or `Create_Instance`)
- Hard-coding provider names (e.g., `SCALEWAY_*`)
- Using non-obvious abbreviations
- Having actions without corresponding log
