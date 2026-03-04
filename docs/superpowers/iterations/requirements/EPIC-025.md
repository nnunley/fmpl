# EPIC-025 — Object System

**Summary:** Object System
**Stories:** STORY-0079, STORY-0080, STORY-0081
**Primary sources:** `specs/fmpl-core.md`, `specs/vm.md`
**Status:** 0/3 done

## STORY-0079

**Epic:** EPIC-025 — Object System
**Title:** Bind magical variables in method call frames

**As a** bootstrap pipeline
**I want** method calls to pre-bind self, parent, caller, user, and args in the new frame
**So that** object methods can access their receiver and call context

**Acceptance criteria:**
- AC-1: LoadSelf returns the ObjectId of the object receiving the method call (frame.this) · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0056`
- AC-2: LoadParent returns the prototype parent of the receiver object, or null if none · impact:`local` · seam:`integration` · scenario:`SCENARIO-0056`
- AC-3: LoadCaller returns the ObjectId from the previous frame's this, or null if none · impact:`local` · seam:`integration` · scenario:`SCENARIO-0056`
- AC-4: LoadArgs returns a List containing all arguments passed to the method · impact:`local` · seam:`integration` · scenario:`SCENARIO-0056`

**Sources:**
- `specs/vm.md:220-249`

**Status:** pending

## STORY-0080

**Epic:** EPIC-025 — Object System
**Title:** Access object properties and spawn new objects

**As a** bootstrap pipeline
**I want** GetProp, SetProp, Spawn, and GetFacet instructions to interact with the ObjectDb
**So that** compiled object-oriented code can read/write properties, create instances, and access facets

**Acceptance criteria:**
- AC-1: GetProp reads a named property from the object at values[object] and stores it at values[ip] · impact:`cross-surface` · seam:`integration`
- AC-2: SetProp writes values[value] to a named property on the object at values[object] · impact:`cross-surface` · seam:`integration`
- AC-3: Spawn creates a new object from a prototype at values[object] with the given args · impact:`cross-surface` · seam:`integration`
- AC-4: GetFacet retrieves a named facet from the object at values[object] · impact:`local` · seam:`integration`

**Sources:**
- `specs/vm.md:107-111`

**Status:** pending

## STORY-0081

**Epic:** EPIC-025 — Object System
**Title:** Provide prototype-based object system

**As a** FMPL program
**I want** an object database with prototype-based objects and Goblins-inspired facets
**So that** programs can create objects with controlled capability exposure

**Acceptance criteria:**
- AC-1: Object, ObjectDb, and ObjectId types are publicly exported · impact:`cross-surface` · seam:`integration`
- AC-2: Objects support facets for capability-controlled member access · impact:`local` · seam:`integration`
- AC-3: Value::Object wraps ObjectId and Value::Facet carries object reference with members · impact:`local` · seam:`unit`

**Sources:**
- `specs/fmpl-core.md:18`
- `specs/fmpl-core.md:37`
- `specs/fmpl-core.md:96`

**Status:** pending
