# Project Document Vault & Task Context: 01 - Concept & Vision

## 1. The Problem

When applying AI Agents to a real-world project (e.g., an internal enterprise repository), a Global Knowledge Base (like SKILL files for frameworks, coding conventions) is not enough. Agents often perform poorly due to the lack of **Business Context**:
- The Agent does not know the current Database Schema of the project (table design, relationships).
- The Agent is unaware of the existing System APIs (endpoints, payloads, responses).
- The Agent does not understand the specific Project Architecture of the current system.
- Some complex Tasks require Design files, User Flows (Sequence Diagrams) for the AI to understand and code accurately.

This deficiency leads to a high degree of **Hallucination**, where the Agent "invents" nonexistent functions, libraries, or endpoints within the project.

## 2. The Solution: Context Layers

To solve this, the **Agentic-Coding** system will divide its context into three distinct layers:

1. **Global Knowledge Base (General Skills - SKILLs)**: How to use frameworks, language conventions. Shared across *all* projects.
2. **Project Document Vault (Project-Specific Knowledge)**: API documentation, Database Schema, System Architecture. Specific to *one* project. Solves the weakness of the Agent not grasping the current Project's structure.
3. **Task Documents (Task-Specific Context)**: Detailed PRD files, UI mockups, or specific error logs for a Task. Injected directly into the *Prompt* when the Agent works on that Task to focus its attention on the immediate problem.

## 3. Architecture Vision

### 3.1. Project Document Vault
- **Nature**: Each Project in the system (Dashboard) will have its own document vault. Users can upload various files or sync from the `.acpms/vault/` folder in the Repository.
- **Supported Document Types**:
  To ensure high-quality vector embeddings and accurate retrieval, the Vault should focus on text-based and structured formats:
  - **Markdown (`.md`)**: The primary and most recommended format. Excellent for System Architecture docs, Coding Guidelines, and Business Rules.
  - **Plain Text (`.txt`)**: General notes or raw logs.
  - **JSON (`.json`)**: Ideal for API specifications (like OpenAPI/Swagger exports) or Mock Data structures. The system can parse the keys/values for better context.
  - **YAML (`.yml`, `.yaml`)**: Good for CI/CD configurations, Docker Compose setups, or Infrastructure as Code (IaC) definitions.
  - **PDF (`.pdf` - Future Phase)**: Requires a text-extraction pipeline before chunking. Useful for legacy enterprise PRDs or technical whitepapers.
  - **Design Files (Figma Links, Images)**: 
    - *In the Vault*: We do NOT support uploading raw image files (`.png`, `.jpg`) or raw design exports directly into the Vault because RAG vector search relies on text. However, users CAN upload a Markdown file containing a **Figma URL** or a detailed text-based **Design Specification**.
    - *In Task Context*: This is where raw design files shine. When creating a specific Task (e.g., "Implement Login Screen"), users can attach the exact `.png` mockup or the Figma frame directly to the Task. The Agent (which natively supports Vision capabilities) will "see" this image when handling the Task.
  - **Architecture Diagrams**:
    - The best way to store diagrams in the Vault is using **Mermaid.js** or **PlantUML** embedded inside a Markdown (`.md`) file. The Agent can perfectly read and understand the relationships between components this way. Raw `.png` diagrams should be avoided in the general Vault.
- **Storage & Retrieval (RAG)**: 
  - Reuse the `sqlite-vec` and `fastembed` engine (already planned for the Global KB).
  - When indexing documents into the DB, each vector (chunk) must store metadata: `project_id`.
  - When an Agent initializes a Session for a Project, it will have a Tool to query the RAG with the condition: `WHERE project_id = '{current_project_id}'`.
- **UI/UX**: 
  - The Project Dashboard interface adds a **"Knowledge Vault"** tab.
  - Allows the project administrator (Super Admin / Tech Lead) to manage, add, and edit this document repository.

### 3.2. Task Context
- **Nature**: Static context attached directly to each specific Task on the Kanban board.
- **UI Integration**: When a User views Task details on the web app, in addition to the Description, the system provides a **"Context Files"** section to attach files or write Markdown directly.
- **Backend Integration (Rust)**: Inject the content of these `Task Contexts` directly into the Agent's System Prompt or User Prompt as soon as the Session is Spawned. No need for random RAG queries, ensuring the Agent does not miss the context.

## 4. Backend & Frontend Integration (Agentic-Coding)

- **Backend (Rust)**: 
  - Add tables `project_documents` (stores metadata & raw files) and `project_document_chunks` (stores `f32` vectors via `sqlite-vec`).
  - Add the `Task Context` structure (can be stored directly in the `metadata` JSON field in the existing `tasks` table or create a new table `task_contexts`).
  - Update the spawning module (e.g., `crates/core/src/agents/spawner.rs`) to append Task Context and provide the RAG schema for Tool Calls.
- **Frontend (Next.js / React)**:
  - Add the `Vault` tab to the Project View screen.
  - Update the Task creation/editing Modal to display an additional `Task Context` section.

## 5. Anti-Noise & Context Optimization

Expanding the context (using the Vault) comes with a massive risk: **Context Noise** and **Context Window Limits**. If the entire project documentation is injected into the Prompt, the AI will become "confused", response times will slow down, and a large number of Tokens will be wasted. Agentic-Coding completely resolves this risk through 3 mechanisms:

### 5.1. Chunking & Retrieval (RAG) instead of Full Reading
- Documents in the Vault are **never** fully injected into the System Prompt.
- Upon upload, documents are automatically chunked semantically and stored as Vectors.
- When the Agent needs to learn about a feature, it is only allowed to retrieve the **Top K** (e.g., 3-5) most semantically similar text segments (Cosine Similarity), limiting the read text to the bare minimum required.

### 5.2. Document Versioning & Stale Data Prevention
- If multiple documents cover the same topic or a document is updated, the system **prioritizes the latest version**.
- When a User uploads a document with the same `title` or `filename`, the system performs an "Upsert" (Overwrite existing).
- The Embedding Pipeline will automatically delete all old Vector Chunks of the previous version before indexing the new ones, ensuring the Agent never reads conflicting or outdated business logic.

### 5.2. Transitioning from "Static Documents" to "Active Tools" (Tool-based Retrieval)
- **Task Context**: Injected directly into the initial **System Prompt** or **User Prompt**, as this is a specific "Command" the Agent must follow immediately (e.g., 1 UI design file, 1 error log).
- **Project Vault (Project Knowledge)**: Provided to the Agent as a **Tool** (e.g., `search_project_vault_tool`). The Agent actively decides when to invoke this tool based on logical reasoning, rather than being forced to read everything beforehand.
  - Ex: The Agent receives the task "Create Login API". It will deduce: *"I need to know how the user table is designed"*, and proactively call `search_project_vault_tool(query="database schema for user table")`.

### 5.3. Granular Meta-data Filtering
- The Vector DB does not just perform Semantic Search, but also utilizes filtering conditions.
- **`WHERE project_id = {current}`**: Ensures the Agent never mistakenly reads documents from another project.
- **`WHERE doc_type IN (Frontend, Backend)`**: Customizable (Future enhancement). If this Agent is tagged as a "Frontend Engineer", it could automatically exclude complex System Architecture Backend documents to keep its focus clear.
